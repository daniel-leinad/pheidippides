use tokio::io::AsyncRead;
use serde::{Deserialize, Serialize};

use pheidippides_messenger::data_access::DataAccess;
use pheidippides_messenger::messenger::Messenger;
use pheidippides_messenger::{MessageId, UserId};
use pheidippides_messenger::authorization::AuthService;
use pheidippides_utils::async_utils;
use pheidippides_utils::serde::form_data;
use pheidippides_utils::utils::CaseInsensitiveString;
use http_server::response::Response;
use http_server::event_source::EventSourceEvent;
use http_server::request::Request;
use pheidippides_utils::http::{get_cookies_hashmap, header_set_cookie};
use crate::{routing, sessions};
use crate::routing::json::MessageJson;

pub fn logout<T: AsyncRead + Unpin>(request: &Request<T>) -> anyhow::Result<Response> {
    let headers = request.headers();
    let cookies = match get_cookies_hashmap(headers) {
        Ok(cookies) => cookies,
        Err(_) => return Ok(Response::BadRequest),
    };

    let session_id = match cookies.get(sessions::SESSION_ID_COOKIE) {
        Some(session_id) => session_id,
        None => return Ok(routing::unauthorized_redirect()),
    };

    sessions::remove_session_info(&session_id)?;
    Ok(routing::unauthorized_redirect())
}

pub async fn authorize<T: AsyncRead + Unpin>(request: &mut Request<T>, app: Messenger<impl DataAccess, impl AuthService>) -> anyhow::Result<Response> {
    let content = request.content().await?;

    #[derive(Deserialize)]
    struct AuthorizationParams {
        login: String,
        password: String,
    }

    let authorization_params: AuthorizationParams =
        match form_data::from_str(&content) {
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
        None => routing::failed_login_response()
    }
}

pub async fn signup<T: AsyncRead + Unpin>(request: &mut Request<T>, app: Messenger<impl DataAccess, impl AuthService>) -> anyhow::Result<Response> {
    let content = request.content().await?;

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


    let signup_params: SignupParams = match serde_json::from_str(&content) {
        Ok(signup_params) => signup_params,
        Err(_) => {
            //TODO handle this case more precisely for client?
            return Ok(Response::BadRequest);
        }
    };

    match app.create_user(&signup_params.login, signup_params.password).await? {
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

pub async fn send_message<D: DataAccess, A, T: AsyncRead + Unpin>(request: &mut Request<T>, app: Messenger<D, A>, receiver: &str) -> anyhow::Result<Response> {

    #[derive(Deserialize)]
    struct SendMessageParams {
        message: String,
    }

    let receiver: UserId = match receiver.parse() {
        Ok(res) => res,
        Err(_) => return Ok(Response::BadRequest),
    };

    let headers = request.headers();
    let authorization = routing::get_authorization(headers)?;

    let user_id = match authorization {
        Some(user_id) => user_id,
        None => return Ok(routing::unauthorized_redirect()),
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

pub async fn subscribe_new_messages<A, T: AsyncRead + Unpin>(request: &Request<T>, app: Messenger<impl DataAccess, A>, params: &str) -> anyhow::Result<Response> {
    #[derive(Deserialize)]
    struct SubscribeNewMessagesParams {
        last_message_id: Option<String>,
    }

    let user_id = match routing::get_authorization(request.headers())? {
        Some(user_id) => user_id,
        None => return Ok(Response::BadRequest)
    };

    let subscribe_new_messages_params: SubscribeNewMessagesParams = match form_data::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::BadRequest),
    };

    let last_message_id_params = match subscribe_new_messages_params.last_message_id {
        Some(s) => {
            match s.parse() {
                Ok(res) => Some(res),
                Err(_) => return Ok(Response::BadRequest),
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

    let subscription = app.subscribe_to_new_messages(user_id, starting_point).await?;

    let stream = async_utils::pipe_unbounded_channel(
        subscription,
        |message| {
            let id = message.id.to_string();
            let data = serde_json::json!(MessageJson::from(message)).to_string();
            let event = None;
            Some(EventSourceEvent { data, id, event })
    });

    Ok(Response::EventSource { retry: None, stream })
}
