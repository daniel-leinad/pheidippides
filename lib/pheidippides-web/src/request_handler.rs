use pheidippides_messenger::data_access::DataAccess;
use tokio::io::AsyncRead;
use pheidippides_messenger::messenger::Messenger;
use web_server::{Request, Response};
use crate::routing;

#[derive(Clone)]
pub struct RequestHandler<D: DataAccess> {
    app: Messenger<D>,
}

impl<D: DataAccess> RequestHandler<D> {
    pub fn new(db_access: D) -> Self {
        RequestHandler { app: Messenger::new(db_access) }
    }
}

#[derive(Debug)]
pub struct RequestHandlerError {
    inner: anyhow::Error,
}

impl From<anyhow::Error> for RequestHandlerError {
    fn from(inner: anyhow::Error) -> Self {
        RequestHandlerError { inner }
    }
}

impl std::fmt::Display for RequestHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::error::Error for RequestHandlerError {}

impl<D: DataAccess, T: AsyncRead + Unpin + Sync + Send> web_server::RequestHandler<Request<T>> for RequestHandler<D> {
    type Error = RequestHandlerError;

    fn handle(self, request: &mut Request<T>) -> impl std::future::Future<Output = anyhow::Result<Response, Self::Error>> + Send {
        routing::route(request, self.app)
    }
}
