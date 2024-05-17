use pheidippides_utils::http::Header;
use tokio::sync::mpsc::UnboundedReceiver;
use crate::event_source::EventSourceEvent;

#[derive(Debug)]
pub enum Response {
    Html {
        content: String,
        headers: Vec<Header>,
    },
    Text{
        text: String,
        headers: Vec<Header>,
    },
    Json{
        content: String,
        headers: Vec<Header>,
    },
    Redirect{
        location: String,
        headers: Vec<Header>
    },
    EventSource{
        retry: Option<i32>,
        stream: UnboundedReceiver<EventSourceEvent>,
    },
    BadRequest,
    InternalServerError,
    Empty,
}

impl Response {
    pub fn is_html(self) -> bool {
        matches!(self, Response::Html {..})
    }
    pub fn is_text(self) -> bool {
        matches!(self, Response::Text {..})
    }
    pub fn is_json(self) -> bool {
        matches!(self, Response::Json {..})
    }
    pub fn is_redirect(self) -> bool {
        matches!(self, Response::Redirect {..})
    }
    pub fn is_event_source(self) -> bool {
        matches!(self, Response::EventSource {..})
    }
    pub fn is_bad_request(self) -> bool {
        matches!(self, Response::BadRequest)
    }
    pub fn is_internal_server_error(self) -> bool {
        matches!(self, Response::InternalServerError)
    }
    pub fn is_empty(self) -> bool {
        matches!(self, Response::Empty)
    }
}