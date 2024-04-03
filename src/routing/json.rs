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

//TODO bad name
#[derive(Deserialize, Debug)]
struct MessagesParams {
    from: Option<MessageId>,
}

pub fn messages_json(request: &Request, db_access: impl db::DbAccess, chat_id: &str, params: &str) -> Result<Response> {

    let query_params: MessagesParams = match serde_form_data::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::BadRequest),
    };

    let headers = get_headers_hashmap(request);
    let user_id = match get_authorization(headers)? {
        Some(res) => res,
        None => return Ok(unauthorized_redirect()),
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