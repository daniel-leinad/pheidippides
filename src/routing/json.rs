use anyhow::Result;
use super::serde_form_data;
use super::db::{self, MessageId};
use crate::http::{Request, Response};
use serde::Deserialize;
use crate::utils::get_headers_hashmap;
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