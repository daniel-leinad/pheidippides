use tokio::io::AsyncRead;
use askama::Template;
use anyhow::Result;

use web_server::{Request, Response};

#[derive(Template)]
#[template(path = "tools/event_source.html")]
struct EventSourcePage {}

pub fn event_source<T: AsyncRead + Unpin>(_request: &Request<T>) -> Result<Response> {
    Ok(Response::Html { content: EventSourcePage{}.render()?, headers: vec![] })
}