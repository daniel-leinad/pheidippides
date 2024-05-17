use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;

use crate::flow_controller::HttpResponseContextExtension;
use crate::routing::json::MessageJson;
use crate::{routing, sessions};
use http_server::event_source::EventSourceEvent;
use http_server::request::Request;
use http_server::response::Response;
use pheidippides_messenger::authorization::AuthService;
use pheidippides_messenger::data_access::DataAccess;
use pheidippides_messenger::messenger::Messenger;
use pheidippides_messenger::{MessageId, UserId};
use pheidippides_utils::async_utils;
use pheidippides_utils::http::{get_cookies_hashmap, header_set_cookie};
use pheidippides_utils::serde::form_data;
use pheidippides_utils::utils::CaseInsensitiveString;

pub fn logout<T: AsyncRead + Unpin>(request: &Request<T>) -> Response {
    let headers = request.headers();
    let cookies = get_cookies_hashmap(headers).or_bad_request()?;

    let session_id = match cookies.get(sessions::SESSION_ID_COOKIE) {
        Some(session_id) => session_id,
        None => return routing::unauthorized_redirect(),
    };

    sessions::remove_session_info(session_id).or_server_error()?;

    routing::unauthorized_redirect()
}

pub async fn authorize<T: AsyncRead + Unpin>(
    request: &mut Request<T>,
    app: Messenger<impl DataAccess, impl AuthService>,
) -> Response {
    let content = request.content().await.or_server_error()?;

    #[derive(Deserialize)]
    struct AuthorizationParams {
        login: String,
        password: String,
    }

    let authorization_params: AuthorizationParams =
        form_data::from_str(&content).or_bad_request()?;

    let user_verification = app
        .verify_user(&authorization_params.login, authorization_params.password)
        .await
        .or_server_error()?;

    match user_verification {
        Some(user_id) => {
            let session_id = sessions::generate_session_id();

            sessions::update_session_info(session_id.clone(), sessions::SessionInfo { user_id })
                .or_server_error()?;

            let location = "/chat".into();
            let headers = vec![header_set_cookie(sessions::SESSION_ID_COOKIE, &session_id)];

            Response::Redirect { location, headers }
        }
        None => routing::failed_login_response().or_server_error()?,
    }
}

pub async fn signup<T: AsyncRead + Unpin>(
    request: &mut Request<T>,
    app: Messenger<impl DataAccess, impl AuthService>,
) -> Response {
    let content = request.content().await.or_server_error()?;

    #[derive(Deserialize)]
    struct SignupParams {
        login: String,
        password: String,
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

    let signup_params: SignupParams = serde_json::from_str(&content).or_bad_request()?;

    match app
        .create_user(&signup_params.login, signup_params.password)
        .await
        .or_server_error()?
    {
        Some(user_id) => {
            let signup_response = serde_json::json!(SignupResponse {
                success: true,
                errors: vec![]
            });
            let session_id = sessions::generate_session_id();
            let headers = vec![header_set_cookie(sessions::SESSION_ID_COOKIE, &session_id)];
            sessions::update_session_info(session_id, sessions::SessionInfo { user_id })
                .or_server_error()?;

            Response::Json {
                content: signup_response.to_string(),
                headers,
            }
        }
        None => {
            let signup_response = SignupResponse {
                success: false,
                errors: vec![SignupError::UsernameTaken],
            };

            Response::Json {
                content: serde_json::json!(signup_response).to_string(),
                headers: vec![],
            }
        }
    }
}

pub async fn send_message<D: DataAccess, A, T: AsyncRead + Unpin>(
    request: &mut Request<T>,
    app: Messenger<D, A>,
    receiver: &str,
) -> Response {
    #[derive(Deserialize)]
    struct SendMessageParams {
        message: String,
    }

    let receiver: UserId = receiver.parse().or_bad_request()?;

    let headers = request.headers();
    let authorization = routing::get_authorization(headers).or_server_error()?;

    let user_id = match authorization {
        Some(user_id) => user_id,
        None => return routing::unauthorized_redirect(),
    };

    let content = &request.content().await.or_server_error()?;
    let params: SendMessageParams = serde_json::from_str(content).or_bad_request()?;

    app.send_message(params.message, user_id, receiver)
        .await
        .or_server_error()?;

    Response::Html {
        content: "ok.".to_owned(),
        headers: Vec::new(),
    }
}

pub async fn subscribe_new_messages<A, T: AsyncRead + Unpin>(
    request: &Request<T>,
    app: Messenger<impl DataAccess, A>,
    params: &str,
) -> Response {
    #[derive(Deserialize)]
    struct SubscribeNewMessagesParams {
        last_message_id: Option<String>,
    }

    let user_id = routing::get_authorization(request.headers())
        .or_server_error()?
        .or_bad_request()?;

    let subscribe_new_messages_params: SubscribeNewMessagesParams =
        form_data::from_str(params).or_bad_request()?;

    let last_message_id_params = match subscribe_new_messages_params.last_message_id {
        Some(s) => Some(s.parse().or_bad_request()?),
        None => None,
    };

    let last_event_id_header = request
        .headers()
        .get(&CaseInsensitiveString::from("Last-Event-ID"));
    let last_message_id_header: Option<MessageId> = match last_event_id_header {
        Some(header_value) => Some(header_value.parse().or_bad_request()?),
        None => None,
    };

    let starting_point = last_message_id_header.or(last_message_id_params);

    let subscription = app
        .subscribe_to_new_messages(user_id, starting_point)
        .await
        .or_server_error()?;

    let stream = async_utils::pipe_unbounded_channel(subscription, |message| {
        let id = message.id.to_string();
        let data = serde_json::json!(MessageJson::from(message)).to_string();
        let event = None;
        Some(EventSourceEvent { data, id, event })
    });

    Response::EventSource {
        retry: None,
        stream,
    }
}
