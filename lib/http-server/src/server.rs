use crate::request::Request;
use crate::response::Response;
use pheidippides_utils::utils::log_internal_error;
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;

pub trait RequestHandler<R>: 'static + Send + Clone {
    type Error: std::error::Error;
    fn handle(
        self,
        request: &mut R,
    ) -> impl std::future::Future<Output = anyhow::Result<Response, Self::Error>> + Send;
}

pub async fn run_server(
    addr: &str,
    request_handler: impl RequestHandler<Request<TcpStream>>,
    cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("Started a server at {addr}");

    loop {
        let (stream, _) = tokio::select! {
            _ = cancellation_token.cancelled() => {
                eprintln!("Shutting down server...");
                break;
            },
            res = listener.accept() => match res {
                Ok(res) => res,
                Err(e) => {
                    log_internal_error(e);
                    continue;
                },
            }
        };

        let request_handler = request_handler.clone();

        tokio::spawn(async move {
            let mut request = match Request::try_from_stream(stream).await {
                Ok(req) => req,
                Err(_) => {
                    // silently ignore all incorrect TCP connections
                    return;
                }
            };

            let response = match request_handler.handle(&mut request).await {
                Ok(response) => response,
                Err(e) => {
                    log_internal_error(e);
                    return;
                }
            };

            if let Err(e) = request.respond(response).await {
                log_internal_error(e)
            };
        });
    }
    eprintln!("Shutting down server...Success");
    Ok(())
}
