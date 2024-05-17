use pheidippides_messenger::data_access::DataAccess;
use tokio::io::AsyncRead;
use pheidippides_messenger::authorization::AuthService;
use pheidippides_messenger::messenger::Messenger;
use http_server::response::Response;
use http_server::request::Request;
use crate::routing;

#[derive(Clone)]
pub struct RequestHandler<D: DataAccess, A> {
    app: Messenger<D, A>,
}

impl<D: DataAccess, A> RequestHandler<D, A> {
    pub fn new(db_access: D, auth_storage: A) -> Self {
        RequestHandler { app: Messenger::new(db_access, auth_storage) }
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

impl<D: DataAccess, A: AuthService, T: AsyncRead + Unpin + Sync + Send> http_server::server::RequestHandler<Request<T>> for RequestHandler<D, A> {
    type Error = RequestHandlerError;

    fn handle(self, request: &mut Request<T>) -> impl std::future::Future<Output = anyhow::Result<Response, Self::Error>> + Send {
        routing::route(request, self.app)
    }
}
