use chrono::DateTime;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;

use pheidippides_utils::serde::form_data as serde_form_data;

use http_server::request::Request;
use http_server::response::Response;

use crate::flow_controller::HttpResponseContextExtension;
use pheidippides_messenger::data_access::DataAccess;
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

pub async fn messages_json<A, T: AsyncRead + Unpin>(
    request: &Request<T>,
    app: Messenger<impl DataAccess, A>,
    chat_id: &str,
    params: &str,
) -> Response {
    let chat_id: UserId = chat_id.parse().or_bad_request()?;
    let query_params: MessagesUrlParams = serde_form_data::from_str(params).or_bad_request()?;

    let starting_from: Option<MessageId> = match query_params.from {
        Some(v) => Some(v.parse().or_bad_request()?),
        None => None,
    };

    let headers = request.headers();
    let user_id = match get_authorization(headers).or_server_error()? {
        Some(res) => res,
        None => {
            let response = MessagesResponse {
                success: false,
                messages: vec![],
                error: Some(MessageResponseError::Unauthorized),
            };
            return Response::Json {
                content: serde_json::json!(response).to_string(),
                headers: vec![],
            };
        }
    };

    let messages: Vec<_> = app
        .fetch_last_messages(&user_id, &chat_id, starting_from.as_ref())
        .await
        .or_server_error()?
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

    Response::Json {
        content: json_response.to_string(),
        headers: vec![],
    }
}
