mod html;
mod json;

use anyhow::{Context, Result};
use super::{sessions, serde_form_data, authorization};
use super::db::{self, UserId};
use super::http::{Request, Response};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
};
use super::utils::{log_internal_error, get_cookies_hashmap, get_headers_hashmap, header_set_cookie};

pub fn handle_request(mut request: Request, db_access: impl db::DbAccess) -> Result<()> {

    let url = request.url();
    let (path, params_anchor) = match url.split_once('?') {
        Some(res) => res,
        None => (url, ""),
    };
    let path = path.to_owned();

    let (params, _anchor) = match params_anchor.split_once('#') {
        Some(res) => res,
        None => (params_anchor, ""),
    };

    let mut path_segments = path
        .split('/')
        .filter(|s| !s.is_empty());


    let method = request.method().clone();
    let query = (
        &method,
        path_segments.next(),
        path_segments.next(),
        path_segments.next(),
        path_segments.next(),
    );

    use tiny_http::Method::*;
    let response = match query {
        (Get, None, ..) => main_page(&request),
        (Get, Some("login"), None, ..) => authorization_page(),
        (Get, Some("logout"), None, ..) => logout(&request),
        (Post, Some("authorize"), None, ..) => authorization(&mut request, db_access),
        (Get, Some("chat"), chat_id, None, ..) => chat_page(&request, db_access, chat_id.map(|s| s.to_owned())),
        (Post, Some("message"), Some(receiver), None, ..) => send_message(&mut request, db_access, &receiver.to_owned()),
        (Get, Some("html"), Some("chats"), None, ..) => html::chats_html_response(&request, db_access),
        (Get, Some("html"), Some("chatsearch"), None, ..) => html::chatsearch_html(db_access, params),
        (Get, Some("json"), Some("messages"), Some(chat_id), None, ..) => json::messages_json(&request, db_access, chat_id, params),
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
        Some(_) => Ok(Response::Redirect{location: "/chat".into(), headers: Vec::new()}),
        None => Ok(unauthorized_redirect()),
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
        None => return Ok(unauthorized_redirect()),
    };

    let username = db_access
        .username(&user_id)?
        .context("Couldn't retrieve username from user_id stored SESSION_INFO")?;

    let chat_page_template = fs::read("chat.html").context("Couldn't open file chat.html")?;
    let chat_page_template =
        String::from_utf8(chat_page_template).context("Invalid utf-8 in chat.html")?;

    let chats_html: String = html::chats_html(&db_access, &user_id)?;

    let chat_page = chat_page_template
        .replace("{username}", &username)
        .replace("{chats}", &chats_html)
        .replace("{chat_id}", &chat_id.unwrap_or_default());

    Ok(Response::HtmlPage {
        bytes: chat_page.as_bytes().to_owned(),
        headers: Vec::new(),
    })
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
        Err(_) => return Ok(unauthorized_redirect()),
    };

    let session_id = match cookies.get(sessions::SESSION_ID_COOKIE) {
        Some(session_id) => session_id,
        None => return Ok(unauthorized_redirect()),
    };

    sessions::remove_session_info(&session_id)?;
    Ok(unauthorized_redirect())
}

#[derive(Deserialize)]
struct AuthorizationParams {
    login: String,
    password: String,
}

fn authorization(request: &mut Request, db_access: impl db::DbAccess) -> Result<Response> {
    let content = request.content()?;

    let authorization_params: AuthorizationParams =
        match serde_form_data::from_str(&content) {
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
        let session_id = sessions::generate_session_id();
        sessions::update_session_info(session_id.clone(), sessions::SessionInfo { user_id })?;
        let location = "/chat".into();
        let headers = vec![header_set_cookie(sessions::SESSION_ID_COOKIE, &session_id)?];

        Ok(Response::Redirect { location , headers })
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
        None => return Ok(unauthorized_redirect()),
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

fn unauthorized_redirect() -> Response {
    Response::Redirect{location: "/login".into(), headers: Vec::new()}
}

fn get_authorization(headers: HashMap<String, String>) -> Result<Option<db::UserId>> {
    let cookies = match get_cookies_hashmap(headers) {
        Ok(cookies) => cookies,
        // TODO handle error
        Err(_) => return Ok(None),
    };

    let session_id = match cookies.get(sessions::SESSION_ID_COOKIE) {
        Some(session_id) => session_id,
        None => return Ok(None),
    };
    let session_info = sessions::get_session_info(session_id)?;
    Ok(session_info.map(|v| v.user_id))
}