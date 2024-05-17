use tokio::io::AsyncRead;
use pheidippides_messenger::data_access::DataAccess;
use pheidippides_messenger::messenger::Messenger;
use http_server::response::Response;
use http_server::request::Request;
use crate::routing;
use crate::routing::html;
use crate::flow_controller::HttpResponseContextExtension;

pub fn main() -> Response {
    Response::Redirect{location: "/chat".into(), headers: Vec::new()}
}

pub async fn chat<D: DataAccess, A, T: AsyncRead + Unpin>(
    request: &Request<T>,
    app: Messenger<D, A>,
    _chat_id: Option<&str>,
) -> Response {

    let headers = request.headers();

    let user_id = match routing::get_authorization(headers).or_server_error()? {
        Some(user_id) => user_id,
        None => return routing::unauthorized_redirect(),
    };

    let chat_page = html::chat_page(&app, &user_id).await.or_server_error()?;

    Response::Html {
        content: chat_page,
        headers: Vec::new(),
    }
}

pub async fn authorization() -> Response {
    let content = html::login_page().or_server_error()?;

    Response::Html {
        content,
        headers: Vec::new(),
    }
}

pub async fn signup() -> Response {
    let content = html::signup_page().or_server_error()?;
    let headers = vec![];

    Response::Html { content , headers }
}
