use anyhow::Result;
use super::serde_form_data;
use super::db::{self, MessageId, UserId};
use crate::db::Message;
use crate::http::{Request, Response};
use serde::{Serialize, Deserialize};
use super::get_authorization;

#[derive(Deserialize, Debug)]
struct MessagesUrlParams {
    from: Option<String>,
}

#[derive(Serialize)]
struct MessagesResponse {
    success: bool,
    messages: Vec<Message>,
    error: Option<MessageResponseError>,
}

#[derive(Serialize)]
enum MessageResponseError {
    Unauthorized,
}

pub async fn messages_json(request: &Request, db_access: impl db::DbAccess, chat_id: &str, params: &str) -> Result<Response> {

    let chat_id: UserId = match chat_id.parse() {
        Ok(chat_id) => chat_id,
        Err(_) => return Ok(Response::BadRequest),
    };

    let query_params: MessagesUrlParams = match serde_form_data::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::BadRequest),
    };

    let starting_from: Option<MessageId> = match query_params.from {
        Some(v) => match v.parse() {
            Ok(v) => Some(v),
            Err(_) => return Ok(Response::BadRequest),
        },
        None => None,
    };

    let headers = request.headers();
    let user_id = match get_authorization(headers)? {
        Some(res) => res,
        None => {
            let response = MessagesResponse { 
                success: false, 
                messages: vec![], 
                error: Some(MessageResponseError::Unauthorized),
            };
            return Ok(Response::Json { content: serde_json::json!(response).to_string(), headers: vec![] })
        },
    };

    let messages: Vec<_> = db_access
        .last_messages(&user_id, &chat_id.to_owned(), starting_from).await?
        .into_iter()
        .rev()
        .collect();
    let response = MessagesResponse {
        success: true,
        messages,
        error: None,
    };
    let json_response = serde_json::json!(response);
    Ok(Response::Json{content: json_response.to_string(), headers: vec![]})
}