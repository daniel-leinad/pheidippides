use pheidippides_utils::utils::Header;
use tokio::sync::mpsc::UnboundedReceiver;
use crate::event_source::EventSourceEvent;

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
