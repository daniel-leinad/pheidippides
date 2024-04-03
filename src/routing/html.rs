use anyhow::{bail, Context, Result};
use clap::Parser;
use super::{sessions, serde_form_data, authorization};
use super::db::{self, MessageId, UserId};
use crate::http::{Header, Request, Response, Server};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::RwLock;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
};
use crate::utils::{log_internal_error, get_cookies_hashmap, get_headers_hashmap, header_set_cookie};
use super::{get_authorization, unauthorized_redirect};

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

pub fn messages_html_response(request: &Request, db_access: impl db::DbAccess, other_user_id: &String) -> Result<Response> {
    let headers = get_headers_hashmap(request);
    let authorization = get_authorization(headers)?;
    let response_string = match authorization {
        Some(user_id) => messages_html(&db_access, &user_id, other_user_id)?,
        None => String::from("Unauthorized"),
    };
    Ok(Response::Text(response_string))
}

pub fn messages_html(db_access: &impl db::DbAccess, user_id: &UserId, other_user_id: &UserId) -> Result<String> {
    let res = db_access
        .last_messages(user_id, other_user_id, None)?
        .iter()
        .rev()
        .map(|msg| HtmlString::from(msg).0)
        .intersperse(String::from("\n"))
        .collect();
    Ok(res)
}

pub fn chats_html_response(request: &Request, db_access: impl db::DbAccess) -> Result<Response> {
    let headers = get_headers_hashmap(request);
    let authorization = get_authorization(headers)?;
    let response_string = match authorization {
        Some(user_id) => chats_html(&db_access, &user_id)?,
        None => String::from("Unauthorized"),
    };
    Ok(Response::Text(response_string))
}

pub fn chats_html(db_access: &impl db::DbAccess, user_id: &UserId) -> Result<String> {
    let res: String = db_access
        .chats(user_id)?
        .iter()
        .map(|chat_info| HtmlString::from(chat_info).0)
        .intersperse(String::from("\n"))
        .collect();
    Ok(res)
}

#[derive(Deserialize)]
struct ChatSearchParams {
    query: String,
}

pub fn chatsearch_html(db_access: impl db::DbAccess, params: &str) -> Result<Response> {

    let search_params: ChatSearchParams = match serde_form_data::from_str(params) {
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