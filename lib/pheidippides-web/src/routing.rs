mod html;
mod json;
mod tools;
mod pages;
mod actions;

use std::collections::HashMap;

use anyhow::Result;
use tokio::io::AsyncRead;
use pheidippides_messenger::authorization::AuthService;

use http_server::{self};
use http_server::request::Request;
use http_server::response::Response;

use pheidippides_utils::utils::{
    CaseInsensitiveString, log_internal_error
};
use pheidippides_messenger::UserId;
use pheidippides_messenger::messenger::Messenger;
use pheidippides_messenger::data_access::DataAccess;
use pheidippides_utils::http::get_cookies_hashmap;

use crate::request_handler::RequestHandlerError;
use crate::sessions;

pub async fn route<T: AsyncRead + Unpin>(request: &mut Request<T>, app: Messenger<impl DataAccess, impl AuthService>) -> Result<Response, RequestHandlerError> {

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

    use http_server::method::Method::*;
    let response = match query {
        (Get, None, ..) => pages::main(),
        (Get, Some("login"), None, ..) => pages::authorization().await,
        (Get, Some("signup"), None, ..) => pages::signup().await,
        (Get, Some("chat"), chat_id, None, ..) => pages::chat(request, app, chat_id).await,
        (Post, Some("signup"), None, ..) => actions::signup(request, app).await,
        (Get, Some("logout"), None, ..) => actions::logout(request),
        (Post, Some("authorize"), None, ..) => actions::authorize(request, app).await,
        (Post, Some("message"), Some(receiver), None, ..) => actions::send_message(request, app, receiver).await,
        (Get, Some("subscribe"), Some("new_messages"), None, ..) => actions::subscribe_new_messages(request, app, params).await,
        (Get, Some("html"), Some("chats"), None, ..) => html::chats_html_response(request, app).await,
        (Get, Some("html"), Some("chatsearch"), None, ..) => html::chatsearch_html(app, params).await,
        (Get, Some("html"), Some("chat"), Some(chat_id), ..) => html::chat_html_response(app, chat_id).await,
        (Get, Some("json"), Some("messages"), Some(chat_id), None, ..) => json::messages_json(request, app, chat_id, params).await,
        (Get, Some("tools"), Some("event_source"), None, ..) => tools::event_source(request),
        (Get, Some("favicon.ico"), None, ..) => Ok(Response::Empty),
        _ => Ok(Response::BadRequest),
    };

    let response = response.unwrap_or_else(|error| {
        log_internal_error(error);
        Response::InternalServerError
    });

    Ok(response)
}

fn failed_login_response() -> Result<Response> {
    let content = html::login_fail_page()?;
    Ok(Response::Html {
        content,
        headers: Vec::new(),
    })
}

fn unauthorized_redirect() -> Response {
    Response::Redirect{location: "/login".into(), headers: Vec::new()}
}

fn get_authorization(headers: &HashMap<CaseInsensitiveString, String>) -> Result<Option<UserId>> {
    let cookies = match get_cookies_hashmap(headers) {
        Ok(cookies) => cookies,
        Err(_) => return Ok(None),
    };

    let session_id = match cookies.get(sessions::SESSION_ID_COOKIE) {
        Some(session_id) => session_id,
        None => return Ok(None),
    };
    let session_info = sessions::get_session_info(session_id)?;
    Ok(session_info.map(|v| v.user_id))
}
