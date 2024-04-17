use tokio::io::AsyncRead;
use askama::Template;
use anyhow::Result;

use crate::http::{Request, Response};

#[derive(Template)]
#[template(path = "tools/event_source.html")]
struct EventSourcePage {}

pub fn event_source<T: AsyncRead + Unpin>(request: &Request<T>) -> Result<Response> {
    Ok(Response::Html { content: EventSourcePage{}.render()?, headers: vec![] })
}