mod html;
mod json;

use anyhow::Result;
use tokio::sync::mpsc::unbounded_channel;
use super::{sessions, serde_form_data};
use super::db;
use crate::app::App;
use crate::authorization;
use crate::db::{DbAccess, MessageId};
use crate::http::{self, EventSourceEvent, Request, Response};
use crate::utils::CaseInsensitiveString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use super::utils::{log_internal_error, get_cookies_hashmap, header_set_cookie};

#[derive(Clone)]
pub struct RequestHandler<D: db::DbAccess> {
    app: App<D>,
}

impl<D: db::DbAccess> RequestHandler<D> {
    pub fn new(db_access: D) -> Self {
        RequestHandler { app: App::new(db_access) }
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
        handle_request(request, self.app)
    }
}

pub async fn handle_request(request: &mut Request, app: App<impl db::DbAccess>) -> Result<Response, RequestHandlerError> {

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
        (Post, Some("signup"), None, ..) => signup(request, app).await,
        (Get, Some("logout"), None, ..) => logout(request),
        (Post, Some("authorize"), None, ..) => authorization(request, app).await,
        (Get, Some("chat"), chat_id, None, ..) => chat_page(request, app, chat_id).await,
        (Post, Some("message"), Some(receiver), None, ..) => send_message(request, app, receiver).await,
        (Get, Some("html"), Some("chats"), None, ..) => html::chats_html_response(request, app).await,
        (Get, Some("html"), Some("chatsearch"), None, ..) => html::chatsearch_html(app, params).await,
        (Get, Some("html"), Some("chat"), Some(chat_id), ..) => html::chat_html_response(app, chat_id).await,
        (Get, Some("json"), Some("messages"), Some(chat_id), None, ..) => json::messages_json(request, app, chat_id, params).await,
        (Get, Some("subscribe"), Some("new_messages"), None, ..) => subscribe_new_messages(request, app, params).await,
        (Get, Some("favicon.ico"), None, ..) => Ok(Response::Empty),
        _ => Ok(Response::BadRequest),
    };

    let response = response.unwrap_or_else(|error| {
        log_internal_error(error);
        Response::InternalServerError
    });

    Ok(response)

    // request.respond(response)
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
    app: App<D>,
    _chat_id: Option<&str>,
) -> Result<Response> {
    
    let headers = request.headers();

    let user_id = match get_authorization(headers)? {
        Some(user_id) => user_id,
        None => return Ok(unauthorized_redirect()),
    };

    let chat_page = html::chat_page(&app, &user_id).await?;

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

async fn authorization(request: &mut Request, app: App<impl db::DbAccess>) -> Result<Response> {
    let content = request.content().await?;

    let authorization_params: AuthorizationParams =
        match serde_form_data::from_str(&content) {
            Ok(authorization_params) => authorization_params,
            Err(_) => {
                //TODO handle this case more precisely for client?
                return Ok(Response::BadRequest);
            }
        };
    
    let user_verification = app.verify_user(&authorization_params.login, authorization_params.password).await?;

    match user_verification {
        Some(user_id) => {
            let session_id = sessions::generate_session_id();
            sessions::update_session_info(session_id.clone(), sessions::SessionInfo { user_id })?;
            let location = "/chat".into();
            let headers = vec![header_set_cookie(sessions::SESSION_ID_COOKIE, &session_id)];

            Ok(Response::Redirect { location , headers })
        }
        None => failed_login_response()
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

async fn signup(request: &mut Request, app: App<impl DbAccess>) -> Result<Response> {
    let content = request.content().await?;
    let auth_params: AuthorizationParams = match serde_json::from_str(&content) {
        Ok(auth_params) => auth_params,
        Err(_) => {
            //TODO handle this case more precisely for client?
            return Ok(Response::BadRequest);
        }
    };

    match app.create_user(&auth_params.login, auth_params.password).await? {
        Some(user_id) => {
            let signup_response = serde_json::json!(SignupResponse{ success: true, errors: vec![] });
            let session_id = sessions::generate_session_id();
            let headers = vec![header_set_cookie(sessions::SESSION_ID_COOKIE, &session_id)];
            sessions::update_session_info(session_id, sessions::SessionInfo{ user_id })?;
            Ok(Response::Json{ content: signup_response.to_string(), headers })
        },
        None => {
            let signup_response = SignupResponse{ success: false, errors: vec![SignupError::UsernameTaken] };
            Ok(Response::Json{content: serde_json::json!(signup_response).to_string(), headers: vec![]})
        },
    }
}

#[derive(Deserialize)]
struct SendMessageParams {
    message: String,
}

async fn send_message<D: db::DbAccess>(request: &mut Request, app: App<D>, receiver: &str) -> Result<Response> {

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

    app.send_message(params.message, user_id, receiver).await?;

    Ok(Response::Html { content: "ok.".to_owned(), headers: Vec::new() })
}

fn failed_login_response() -> Result<Response> {
    let content = html::login_fail_page()?;
    Ok(Response::Html {
        content,
        headers: Vec::new(),
    })
}

#[derive(Deserialize)]
struct SubscribeNewMessagesParams<'a> {
    last_message_id: Option<&'a str>,
}

async fn subscribe_new_messages(request: &Request, app: App<impl DbAccess>, params: &str) -> Result<Response> {
    let user_id = match get_authorization(request.headers())? {
        Some(user_id) => user_id, 
        None => return Ok(Response::BadRequest)
    };

    let subscribe_new_messages_params: SubscribeNewMessagesParams = match serde_form_data::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::BadRequest),
    };

    let last_message_id_params = match subscribe_new_messages_params.last_message_id {
        Some(s) => {
            match s.parse() {
                Ok(res) => Some(res),
                Err(_) => return Ok(Response::BadRequest)
            }
        },
        None => None,
    };

    let last_message_id_header: Option<MessageId> = match request.headers().get(&CaseInsensitiveString::from("Last-Event-ID")) {
        Some(header_value) => {
            match header_value.parse() {
                Ok(last_event_id) => Some(last_event_id),
                Err(_) => return Ok(Response::BadRequest),
            }
        },
        None => None,
    };

    let starting_point = last_message_id_header.or(last_message_id_params);

    let mut subscription = app.subscribe_new_messages(user_id, dbg!(starting_point)).await?;

    let (sender, receiver) = unbounded_channel();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = sender.closed() => {
                    break;
                },

                message_res = subscription.recv() => {
                    let message = match message_res {
                        Ok(message) => message,
                        Err(e) => {
                            log_internal_error(e);
                            break
                        },
                    };
                    let event_source_event = EventSourceEvent { 
                        data: serde_json::json!(message).to_string(),
                        id: message.id.to_string(), 
                        event: None,
                    };
                    if let Err(_) = sender.send(event_source_event) {
                        // Client has disconnected
                        break
                    }
                },
            }
        }
    });

    Ok(Response::EventSource { retry: None, stream: receiver })
}

fn unauthorized_redirect() -> Response {
    Response::Redirect{location: "/login".into(), headers: Vec::new()}
}

// TODO return 'static reference for optimisation?
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