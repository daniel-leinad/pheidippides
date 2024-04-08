mod html;
mod json;

use anyhow::{Context, Result};
use super::{sessions, serde_form_data, authorization};
use super::db;
use crate::db::DbAccess;
use crate::http::{self, Request, Response};
use crate::utils::CaseInsensitiveString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::utils::{log_internal_error, get_cookies_hashmap, header_set_cookie};

#[derive(Clone)]
pub struct RequestHandler<D: db::DbAccess> {
    db_access: D,
}

impl<D: db::DbAccess> RequestHandler<D> {
    pub fn new(db_access: D) -> Self {
        RequestHandler { db_access }
    }
}

#[derive(Debug)]
pub struct RequestHandlerError {
    inner: anyhow::Error,
}

impl From<anyhow::Error> for RequestHandlerError {
    fn from(inner: anyhow::Error) -> Self {
        RequestHandlerError { inner }
    }
}

impl std::fmt::Display for RequestHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::error::Error for RequestHandlerError {}

impl<D: db::DbAccess> http::RequestHandler for RequestHandler<D> {
    type Error = RequestHandlerError;

    fn handle(self, request: &mut Request) -> impl std::future::Future<Output = Result<Response, Self::Error>> + Send {
        handle_request(request, self.db_access)
    }
}

pub async fn handle_request(request: &mut Request, db_access: impl db::DbAccess) -> Result<Response, RequestHandlerError> {

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

    use http::Method::*;
    let response = match query {
        (Get, None, ..) => main_page(request),
        (Get, Some("login"), None, ..) => authorization_page().await,
        (Get, Some("signup"), None, ..) => signup_page().await,
        (Post, Some("signup"), None, ..) => signup(request, db_access).await,
        (Get, Some("logout"), None, ..) => logout(request),
        (Post, Some("authorize"), None, ..) => authorization(request, db_access).await,
        (Get, Some("chat"), chat_id, None, ..) => chat_page(request, db_access, chat_id).await,
        (Post, Some("message"), Some(receiver), None, ..) => send_message(request, db_access, receiver).await,
        (Get, Some("html"), Some("chats"), None, ..) => html::chats_html_response(request, db_access).await,
        (Get, Some("html"), Some("chatsearch"), None, ..) => html::chatsearch_html(db_access, params).await,
        (Get, Some("json"), Some("messages"), Some(chat_id), None, ..) => json::messages_json(request, db_access, chat_id, params).await,
        (Get, Some("favicon.ico"), None, ..) => Ok(Response::Empty),
        (Get, Some("users"), None, ..) => get_users_debug(db_access).await,
        _ => Ok(Response::BadRequest),
    };

    let response = response.unwrap_or_else(|error| {
        log_internal_error(error);
        Response::InternalServerError
    });

    Ok(response)

    // request.respond(response)
}

async fn get_users_debug(db_access: impl db::DbAccess) -> Result<Response> {
    let mut res = String::new();

    for user_info in db_access.users().await.context("Couldn't fetch users")? {
        res.push_str(&user_info.0.to_string());
        res.push_str(" ");
        res.push_str(&user_info.1);
        res.push_str("\n");
    };

    Ok(Response::Text { text: res, headers: vec![] })

}

fn main_page(request: &Request) -> Result<Response> {
    let headers = request.headers();

    match get_authorization(headers)? {
        Some(_) => Ok(Response::Redirect{location: "/chat".into(), headers: Vec::new()}),
        None => Ok(unauthorized_redirect()),
    }
}

async fn chat_page<D: DbAccess>(
    request: &Request,
    db_access: D,
    _chat_id: Option<&str>,
) -> Result<Response> {
    

    let headers = request.headers();

    let user_id = match get_authorization(headers)? {
        Some(user_id) => user_id,
        None => return Ok(unauthorized_redirect()),
    };

    let chat_page = html::chat_page(&db_access, &user_id).await?;

    Ok(Response::Html {
        content: chat_page,
        headers: Vec::new(),
    })
}

async fn authorization_page() -> Result<Response> {
    let content = html::login_page()?;
    Ok(Response::Html {
        content,
        headers: Vec::new(),
    })
}

async fn signup_page() -> Result<Response> {
    let content = html::signup_page()?;
    let headers = vec![];
    Ok(Response::Html { content , headers })
}

fn logout(request: &Request) -> Result<Response> {
    let headers = request.headers();
    let cookies = match get_cookies_hashmap(headers) {
        Ok(cookies) => cookies,
        Err(_) => return Ok(Response::BadRequest),
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

async fn authorization(request: &mut Request, db_access: impl db::DbAccess) -> Result<Response> {
    let content = request.content().await?;

    let authorization_params: AuthorizationParams =
        match serde_form_data::from_str(&content) {
            Ok(authorization_params) => authorization_params,
            Err(_) => {
                //TODO handle this case more precisely for client?
                return Ok(Response::BadRequest);
            }
        };

    let user_id = match db_access
        .user_id(&authorization_params.login).await
        .with_context(|| format!("Couldn't fetch user_id of {}", &authorization_params.login))?
    {
        Some(user_id) => user_id,
        None => return failed_login_response().await,
    };

    if authorization
        ::verify_user(&user_id, authorization_params.password, &db_access).await
        .with_context(|| format!("Authorization error: couldn't verify user {}", &user_id))? {
        let session_id = sessions::generate_session_id();
        sessions::update_session_info(session_id.clone(), sessions::SessionInfo { user_id })?;
        let location = "/chat".into();
        let headers = vec![header_set_cookie(sessions::SESSION_ID_COOKIE, &session_id)];

        Ok(Response::Redirect { location , headers })
    } else {
        failed_login_response().await
    }
}

#[derive(Serialize)]
struct SignupResponse {
    success: bool,
    errors: Vec<SignupError>,
}

#[derive(Serialize)]
enum SignupError {
    UsernameTaken,
}

async fn signup(request: &mut Request, db_access: impl db::DbAccess) -> Result<Response> {
    let content = request.content().await?;
    let auth_params: AuthorizationParams = match serde_json::from_str(&content) {
        Ok(auth_params) => auth_params,
        Err(_) => {
            //TODO handle this case more precisely for client?
            return Ok(Response::BadRequest);
        }
    };

    let user_id = match db_access
        .create_user(&auth_params.login).await
        .with_context(|| format!("Couldn't create user {}", &auth_params.login))? {
        Some(user_id) => user_id,
        None => {
            let signup_response = SignupResponse{ success: false, errors: vec![SignupError::UsernameTaken] };
            return Ok(Response::Json{content: serde_json::json!(signup_response).to_string(), headers: vec![]})
        },
    };

    authorization::create_user(&user_id, auth_params.password, &db_access).await.with_context(
        || format!("Authoriazation error: couldn't create user {}", &auth_params.login))?;

    let signup_response = serde_json::json!(SignupResponse{ success: true, errors: vec![] });
    let session_id = sessions::generate_session_id();
    let headers = vec![header_set_cookie(sessions::SESSION_ID_COOKIE, &session_id)];
    sessions::update_session_info(session_id, sessions::SessionInfo{ user_id })?;
    Ok(Response::Json{ content: signup_response.to_string(), headers })
}

#[derive(Deserialize)]
struct SendMessageParams {
    message: String,
}

async fn send_message<D: db::DbAccess>(request: &mut Request, db_access: D, receiver: &str) -> Result<Response> {

    let receiver: db::UserId = match receiver.parse() {
        Ok(res) => res,
        Err(_) => return Ok(Response::BadRequest),
    };

    let headers = request.headers();
    let authorization = get_authorization(headers)?;

    let user_id = match authorization {
        Some(user_id) => user_id,
        None => return Ok(unauthorized_redirect()),
    };

    let params: SendMessageParams = match serde_json::from_str(&request.content().await?) {
        Ok(params) => params,
        Err(_) => {
            //TODO handle this case more precisely for client?
            return Ok(Response::BadRequest);
        },
    };

    db_access.create_message(params.message, &user_id, &receiver).await.with_context(|| format!("Couldn't create message from {user_id} to {receiver}"))?;

    Ok(Response::Html { content: "ok.".to_owned(), headers: Vec::new() })
}

async fn failed_login_response() -> Result<Response> {
    let content = html::login_fail_page()?;
    Ok(Response::Html {
        content,
        headers: Vec::new(),
    })
}

fn unauthorized_redirect() -> Response {
    Response::Redirect{location: "/login".into(), headers: Vec::new()}
}

fn get_authorization(headers: &HashMap<CaseInsensitiveString, String>) -> Result<Option<db::UserId>> {
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