use tokio::io::AsyncRead;
use pheidippides_messenger::data_access::DataAccess;
use pheidippides_messenger::messenger::Messenger;
use http_server::{Request, Response};
use crate::routing;
use crate::routing::html;

pub fn main() -> anyhow::Result<Response> {
    Ok(Response::Redirect{location: "/chat".into(), headers: Vec::new()})
}

pub async fn chat<D: DataAccess, A, T: AsyncRead + Unpin>(
    request: &Request<T>,
    app: Messenger<D, A>,
    _chat_id: Option<&str>,
) -> anyhow::Result<Response> {

    let headers = request.headers();

    let user_id = match routing::get_authorization(headers)? {
        Some(user_id) => user_id,
        None => return Ok(routing::unauthorized_redirect()),
    };

    let chat_page = html::chat_page(&app, &user_id).await?;

    Ok(Response::Html {
        content: chat_page,
        headers: Vec::new(),
    })
}

pub async fn authorization() -> anyhow::Result<Response> {
    let content = html::login_page()?;
    Ok(Response::Html {
        content,
        headers: Vec::new(),
    })
}

pub async fn signup() -> anyhow::Result<Response> {
    let content = html::signup_page()?;
    let headers = vec![];
    Ok(Response::Html { content , headers })
}
