use askama::Template;
use tokio::io::AsyncRead;

use crate::flow_controller::HttpResponseContextExtension;
use http_server::request::Request;
use http_server::response::Response;

#[derive(Template)]
#[template(path = "tools/event_source.html")]
struct EventSourcePage {}

pub fn event_source<T: AsyncRead + Unpin>(_request: &Request<T>) -> Response {
    let content = EventSourcePage {}.render().or_server_error()?;
    let headers = vec![];

    Response::Html { content, headers }
}
