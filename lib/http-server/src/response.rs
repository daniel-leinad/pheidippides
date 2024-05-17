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
        match self {
            Response::Html {..} => true,
            _ => false,
        }
    }
    pub fn is_text(self) -> bool {
        match self {
            Response::Text {..} => true,
            _ => false,
        }
    }
    pub fn is_json(self) -> bool {
        match self {
            Response::Json {..} => true,
            _ => false,
        }
    }
    pub fn is_redirect(self) -> bool {
        match self {
            Response::Redirect {..} => true,
            _ => false,
        }
    }
    pub fn is_event_source(self) -> bool {
        match self {
            Response::EventSource {..} => true,
            _ => false,
        }
    }
    pub fn is_bad_request(self) -> bool {
        match self {
            Response::BadRequest => true,
            _ => false,
        }
    }
    pub fn is_internal_server_error(self) -> bool {
        match self {
            Response::InternalServerError => true,
            _ => false,
        }
    }
    pub fn is_empty(self) -> bool {
        match self {
            Response::Empty => true,
            _ => false,
        }
    }
}