#![feature(slice_split_once)]
#![feature(iter_intersperse)]

mod authorization;
mod db;
mod http;
mod serde_query_string_params;

use anyhow::{bail, Context, Result};
use clap::Parser;
use db::{MessageId, UserId};
use http::{Header, Request, Response, Server};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::RwLock;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
};

type SessionId = String;

const SESSION_ID_COOKIE: &str = "_pheidippides_sid";

#[derive(Clone)]
struct SessionInfo {
    user_id: db::UserId,
}

static SESSION_INFO: Lazy<RwLock<HashMap<SessionId, SessionInfo>>> = Lazy::new(|| RwLock::new(HashMap::new()));

struct HtmlString(String);

impl From<&db::Message> for HtmlString {
    fn from(value: &db::Message) -> Self {
        let class = match value.message_type {
            db::MessageType::In => "messageIn",
            db::MessageType::Out => "messageOut",
        };
        let msg = &value.message;
        HtmlString(format!("<div class=\"{class}\">{msg}</div>"))
    }
}

impl From<&db::ChatInfo> for HtmlString {
    fn from(value: &db::ChatInfo) -> Self {
        let id = &value.id;
        let username = &value.username;
        HtmlString(format!(
            "<div class=\"chat\" id=\"chat_{id}\" onclick=\"chatWith({id})\">{username}</div>"
        ))
    }
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    host: String,
    #[arg(short, long)]
    port: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let host = args.host;
    let port = args.port;
    let addr = format!("{host}:{port}");

    let http_server: Server = match Server::http(&addr) {
        Ok(server) => {
            eprintln!("Started a server at {addr}");
            server
        }
        Err(_) => bail!("Couldn't start a server"),
    };

    let db_access = db::mock::Db::new();

    for request in http_server.incoming_requests() {
        match handle_request(request.into(), db_access.clone()) {
            Ok(()) => {}
            Err(e) => log_internal_error(e),
        }
    }
    Ok(())
}

fn handle_request(mut request: Request, db_access: impl db::DbAccess) -> Result<()> {
    use tiny_http::Method::*;

    //TODO use url crate for this
    let url = request.url();
    let (path, params_anchor) = match url.split_once('?') {
        Some(res) => res,
        None => (url, ""),
    };

    let (params, _anchor) = match params_anchor.split_once('#') {
        Some(res) => res,
        None => (params_anchor, ""),
    };

    let path: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.into())
        .collect();

    let method = request.method().clone();
    let query = (
        &method,
        path.get(0).map(|s| s.as_str()),
        path.get(1).map(|s| s.as_str()),
        path.get(2).map(|s| s.as_str()),
        path.get(3).map(|s| s.as_str()),
    );

    let response = match query {
        (Get, None, ..) => main_page(&request),
        (Get, Some("login"), None, ..) => authorization_page(),
        (Get, Some("logout"), None, ..) => logout(&request),
        (Post, Some("authorize"), None, ..) => authorization(&mut request, db_access),
        (Get, Some("chat"), chat_id, None, ..) => chat_page(&request, db_access, chat_id.map(|s| s.to_owned())),
        (Post, Some("message"), Some(receiver), None, ..) => send_message(&mut request, db_access, &receiver.to_owned()),
        (Get, Some("html"), Some("messages"), Some(chat_id), None, ..) => messages_html_response(&request, db_access, &chat_id.to_owned()),
        (Get, Some("html"), Some("chats"), None, ..) => chats_html_response(&request, db_access),
        (Get, Some("html"), Some("chatsearch"), None, ..) => chatsearch_html(db_access, params),
        (Get, Some("json"), Some("messages"), Some(chat_id), None, ..) => messages_json(&request, db_access, chat_id, params),
        (Get, Some("favicon.ico"), None, ..) => Ok(Response::Empty),
        _ => Ok(Response::BadRequest),
    };

    let response = response.unwrap_or_else(|error| {
        log_internal_error(error);
        Response::InternalServerError
    });

    request.respond(response)
}

fn main_page(request: &Request) -> Result<Response> {
    let headers = get_headers_hashmap(request);

    match get_authorization(headers)? {
        Some(_) => Ok(Response::Redirect("/chat".into())),
        None => Ok(Response::Redirect("/login".into())),
    }
}

fn chat_page(
    request: &Request,
    db_access: impl db::DbAccess,
    chat_id: Option<db::UserId>,
) -> Result<Response> {
    let headers = get_headers_hashmap(request);

    let user_id = match get_authorization(headers)? {
        Some(user_id) => user_id,
        None => return Ok(Response::Redirect("/login".into())),
    };

    let username = db_access
        .username(&user_id)?
        .context("Couldn't retrieve username from user_id stored SESSION_INFO")?;

    let chat_page_template = fs::read("chat.html").context("Couldn't open file chat.html")?;
    let chat_page_template =
        String::from_utf8(chat_page_template).context("Invalid utf-8 in chat.html")?;

    let chats_html: String = chats_html(&db_access, &user_id)?;

    let messages_html: String = match &chat_id {
        Some(other_user_id) => messages_html(&db_access, &user_id, other_user_id)?,
        None => String::new(),
    };

    let chat_page = chat_page_template
        .replace("{username}", &username)
        .replace("{chats}", &chats_html)
        .replace("{messages}", &messages_html)
        .replace("{chat_id}", &chat_id.unwrap_or_default());

    Ok(Response::HtmlPage {
        bytes: chat_page.as_bytes().to_owned(),
        headers: Vec::new(),
    })
}

fn messages_html_response(request: &Request, db_access: impl db::DbAccess, other_user_id: &String) -> Result<Response> {
    let headers = get_headers_hashmap(request);
    let authorization = get_authorization(headers)?;
    let response_string = match authorization {
        Some(user_id) => messages_html(&db_access, &user_id, other_user_id)?,
        None => String::from("Unauthorized"),
    };
    Ok(Response::Text(response_string))
}

fn messages_html(db_access: &impl db::DbAccess, user_id: &UserId, other_user_id: &UserId) -> Result<String> {
    let res = db_access
        .last_messages(user_id, other_user_id, None)?
        .iter()
        .rev()
        .map(|msg| HtmlString::from(msg).0)
        .intersperse(String::from("\n"))
        .collect();
    Ok(res)
}

fn chats_html_response(request: &Request, db_access: impl db::DbAccess) -> Result<Response> {
    let headers = get_headers_hashmap(request);
    let authorization = get_authorization(headers)?;
    let response_string = match authorization {
        Some(user_id) => chats_html(&db_access, &user_id)?,
        None => String::from("Unauthorized"),
    };
    Ok(Response::Text(response_string))
}

fn chats_html(db_access: &impl db::DbAccess, user_id: &UserId) -> Result<String> {
    let res: String = db_access
        .chats(user_id)?
        .iter()
        .map(|chat_info| HtmlString::from(chat_info).0)
        .intersperse(String::from("\n"))
        .collect();
    Ok(res)
}

//TODO bad name
#[derive(Deserialize, Debug)]
struct MessagesParams {
    from: Option<MessageId>,
}

fn messages_json(request: &Request, db_access: impl db::DbAccess, chat_id: &str, params: &str) -> Result<Response> {
    let query_params: MessagesParams = match serde_query_string_params::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::BadRequest),
    };

    let headers = get_headers_hashmap(request);
    let user_id = match get_authorization(headers)? {
        Some(res) => res,
        None => return Ok(Response::Redirect("/login".into())),
    };

    let messages: Vec<_> = db_access
        .last_messages(&user_id, &chat_id.to_owned(), query_params.from)?
        .into_iter()
        .rev()
        .collect();
    let json_messages = serde_json::json!(messages);
    //TODO add special type json?
    Ok(Response::Text(json_messages.to_string()))
}

fn authorization_page() -> Result<Response> {
    let mut authorization_page_file =
        File::open("login.html").context("Couldn't open file login.html")?;
    let mut bytes = Vec::new();
    authorization_page_file
        .read_to_end(&mut bytes)
        .context("Couldn't read file login.html")?;
    return Ok(Response::HtmlPage {
        bytes,
        headers: Vec::new(),
    });
}

fn logout(request: &Request) -> Result<Response> {
    let headers = get_headers_hashmap(request);
    let cookies = match get_cookies_hashmap(headers) {
        Ok(cookies) => cookies,
        //TODO handle error?
        Err(_) => return Ok(Response::Redirect("/login".into())),
    };

    let session_id = match cookies.get(SESSION_ID_COOKIE) {
        Some(session_id) => session_id,
        None => return Ok(Response::Redirect("/login".into())),
    };

    remove_session_info(&session_id)?;
    Ok(Response::Redirect("/login".into()))
}

#[derive(Deserialize)]
struct AuthorizationParams {
    login: String,
    password: String,
}

fn authorization(request: &mut Request, db_access: impl db::DbAccess) -> Result<Response> {
    let content = request.content()?;

    let authorization_params: AuthorizationParams =
        match serde_query_string_params::from_str(&content) {
            Ok(authorization_params) => authorization_params,
            Err(_) => {
                //TODO handle this case more precisely for client?
                return Ok(Response::BadRequest);
            }
        };

    let user_id = match db_access.user_id(&authorization_params.login)? {
        Some(user_id) => user_id,
        None => return failed_login_response(),
    };

    if authorization::validate_user_info(&user_id, &authorization_params.password) {
        let session_id = generate_session_id();
        update_session_info(session_id.clone(), SessionInfo { user_id })?;
        let mut bytes = Vec::new();
        File::open("login_success.html")
            .context("Couldn't open file login_success.html")?
            .read_to_end(&mut bytes)
            .context("Couldn't read file login_success.html")?;
        let headers = vec![header_set_cookie(SESSION_ID_COOKIE, &session_id)?];

        Ok(Response::HtmlPage { bytes, headers })
    } else {
        failed_login_response()
    }
}

#[derive(Deserialize)]
struct SendMessageParams {
    message: String,
}

fn send_message(request: &mut Request, db_access: impl db::DbAccess, receiver: &UserId) -> Result<Response> {

    let headers = get_headers_hashmap(request);
    let authorization = get_authorization(headers)?;

    let user_id = match authorization {
        Some(user_id) => user_id,
        None => return Ok(Response::Redirect("/login".into())),
    };

    let params: SendMessageParams = match serde_json::from_str(&request.content()?) {
        Ok(params) => params,
        Err(_) => {
            //TODO handle this case more precisely for client?
            return Ok(Response::BadRequest);
        },
    };

    db_access.create_message(params.message, &user_id, receiver)?;

    Ok(Response::HtmlPage { bytes: b"ok.".to_vec(), headers: Vec::new() })
}

fn failed_login_response() -> Result<Response> {
    let mut bytes = Vec::new();
    let mut file = File::open("login_fail.html").context("Could not open file login_fail.html")?;
    file.read_to_end(&mut bytes)
        .context("Couldn't read file login_fail.html")?;
    Ok(Response::HtmlPage {
        bytes,
        headers: Vec::new(),
    })
}

#[derive(Deserialize)]
struct ChatSearchParams {
    query: String,
}

fn chatsearch_html(db_access: impl db::DbAccess, params: &str) -> Result<Response> {

    let search_params: ChatSearchParams = match serde_query_string_params::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::Empty),
    };

    let chats_html: String = db_access
        .find_chats(&search_params.query)?
        .into_iter()
        .map(|chat_info| HtmlString::from(&chat_info).0)
        .intersperse("\n".to_owned())
        .collect();

    Ok(Response::Text(chats_html))

}

fn generate_session_id() -> SessionId {
    uuid::Uuid::new_v4().into()
}

fn update_session_info(session_id: SessionId, session_info: SessionInfo) -> Result<()> {
    match SESSION_INFO.write() {
        Ok(mut session_info_write_lock) => {
            session_info_write_lock.insert(session_id, session_info);
            Ok(())
        }
        Err(e) => {
            bail!("Could not lock SESSION_INFO global for write: {}", e)
        }
    }
}

fn get_session_info(session_id: &SessionId) -> Result<Option<SessionInfo>> {
    let res = match SESSION_INFO.read() {
        Ok(session_info_read_lock) => session_info_read_lock.get(session_id).map(|v| v.clone()),
        Err(e) => {
            bail!("Could not lock SESSION_INFO global for read: {}", e)
        }
    };
    Ok(res)
}

fn remove_session_info(session_id: &SessionId) -> Result<()> {
    match SESSION_INFO.write() {
        Ok(mut session_info_write_lock) => {
            session_info_write_lock.remove(session_id);
        },
        Err(e) => bail!("Could not lock SESSION_INFO global for write: {}", e),
    }
    Ok(())
}

fn get_authorization(headers: HashMap<String, String>) -> Result<Option<db::UserId>> {
    let cookies = match get_cookies_hashmap(headers) {
        Ok(cookies) => cookies,
        // TODO handle error
        Err(_) => return Ok(None),
    };

    let session_id = match cookies.get(SESSION_ID_COOKIE) {
        Some(session_id) => session_id,
        None => return Ok(None),
    };
    let session_info = get_session_info(session_id)?;
    Ok(session_info.map(|v| v.user_id))
}

fn get_headers_hashmap(request: &Request) -> HashMap<String, String> {
    let headers = {
        let mut res: HashMap<String, String> = HashMap::new();
        for header in request.headers() {
            res.insert(
                header.field.as_str().as_str().into(),
                header.value.clone().into(),
            );
        }
        res
    };
    headers
}

enum CookieParsingError {
    IncorrectHeader,
}

fn get_cookies_hashmap(
    headers: HashMap<String, String>,
) -> Result<HashMap<String, String>, CookieParsingError> {
    let mut res = HashMap::new();
    if let Some(cookie_list) = headers.get("Cookie") {
        for cookie in cookie_list.split("; ") {
            let (key, value) = match cookie.split_once('=') {
                Some(key_value) => key_value,
                None => return Err(CookieParsingError::IncorrectHeader),
            };
            res.insert(key.into(), value.into());
        }
    }
    Ok(res)
}

fn header_set_cookie(key: &str, value: &str) -> Result<Header> {
    match Header::from_bytes("Set-Cookie", format!("{key}={value}")) {
        Ok(header) => Ok(header),
        Err(()) => bail!("Couldn'r create header Set-Cookie {key}={value}"),
    }
}

fn log_internal_error(error: impl std::fmt::Display) {
    eprintln!("SERVER ERROR: {:#}", error);
}
