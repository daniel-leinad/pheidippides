use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;
use chrono::DateTime;

use pheidippides_utils::serde::form_data as serde_form_data;

use web_server::{Request, Response};

use pheidippides_messenger::data_access::{self};
use pheidippides_messenger::messenger::Messenger;
use pheidippides_messenger::{Message, MessageId, UserId};

use crate::routing::get_authorization;


#[derive(Serialize)]
pub struct MessageJson {
    #[serde(serialize_with = "pheidippides_utils::serde::serialize_uuid")]
    pub id: MessageId,
    #[serde(serialize_with = "pheidippides_utils::serde::serialize_uuid")]
    pub from: UserId,
    #[serde(serialize_with = "pheidippides_utils::serde::serialize_uuid")]
    pub to: UserId,
    pub message: String,
    #[serde(serialize_with = "pheidippides_utils::serde::serialize_datetime")]
    pub timestamp: DateTime<chrono::Utc>,
}

impl From<Message> for MessageJson {
    fn from(message: Message) -> Self {
        Self {
            id: message.id,
            from: message.from,
            to: message.to,
            message: message.message,
            timestamp: message.timestamp,
        }
    }
}

#[derive(Deserialize, Debug)]
struct MessagesUrlParams {
    from: Option<String>,
}

#[derive(Serialize)]
struct MessagesResponse {
    success: bool,
    messages: Vec<MessageJson>,
    error: Option<MessageResponseError>,
}

#[derive(Serialize)]
enum MessageResponseError {
    Unauthorized,
}

pub async fn messages_json<T: AsyncRead + Unpin>(request: &Request<T>, app: Messenger<impl data_access::DataAccess>, chat_id: &str, params: &str) -> Result<Response> {

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

    let messages: Vec<_> = app
        .fetch_last_messages(&user_id, &chat_id, starting_from).await?
        .into_iter()
        .map(|message| message.into())
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