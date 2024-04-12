mod http_response;

use std::{collections::HashMap, str::FromStr};

use anyhow::{Result, Context, bail};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Interest}, net::TcpStream, sync::mpsc::UnboundedReceiver};
use tokio_util::sync::CancellationToken;

use crate::utils::{self, log_internal_error, CaseInsensitiveString};
use http_response::{HttpResponseBuilder, HttpStatusCode};

pub type Header = (CaseInsensitiveString, String);

pub struct Request {
    reader: BufReader<TcpStream>,
    method: Method,
    url: String,
    headers: HashMap<CaseInsensitiveString, String>,
}

#[derive(Debug, Clone, Copy)]
pub enum Method {
    Get,
    Put,
    Post,
    Delete,
    Patch,
    Head,
    Options,
    Trace,
    Connect,
}

#[derive(Debug)]
pub enum MethodParseError {
    IncorrectMethod,
}

impl std::fmt::Display for MethodParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Incorrect method")
    }
}

impl std::error::Error for MethodParseError {}

impl FromStr for Method {
    type Err = MethodParseError;

    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "get" => Ok(Self::Get),
            "put" => Ok(Self::Put),
            "post" => Ok(Self::Post),
            "delete" => Ok(Self::Delete),
            "patch" => Ok(Self::Patch),
            "head" => Ok(Self::Head),
            "options" => Ok(Self::Options),
            "trace" => Ok(Self::Trace),
            "connect" => Ok(Self::Connect),
            _ => Err(MethodParseError::IncorrectMethod)
        }
    }
}

impl Request {
    pub async fn try_from_stream(stream: TcpStream) -> Result<Self> {
        let mut reader = BufReader::new(stream);

        let mut first_line = String::new();
        reader.read_line(&mut first_line).await.context("Could not read line")?;
        let mut first_line_split = first_line.split_whitespace();
        let context = || format!("Could not parse first line: {first_line}");
        let method: Method = first_line_split.next().with_context(context)?.parse()?;
        let url = first_line_split.next().with_context(context)?.to_owned();

        let mut headers = HashMap::new();
        loop {
            let mut next_line = String::new();
            reader.read_line(&mut next_line).await.context("Could not read line")?;
            if next_line == "\r\n" {
                break;
            } else {
                let (key, value) = next_line.split_once(": ").with_context(|| format!("Incorrect header: {next_line}"))?;
                headers.insert(key.into(), value.trim_end().to_string());
            }
        };
        
        Ok(Request { reader , method, url, headers})
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn method(&self) -> Method {
        self.method
    }

    pub fn headers(&self) -> &HashMap<CaseInsensitiveString, String> {
        &self.headers
    }

    pub async fn content(&mut self) -> Result<String> {
        let header_name: CaseInsensitiveString = "content-length".into();
        let content_length: usize = self
            .headers()
            .get(&header_name)
            .context("Content-Length header is missing")?
            .parse()
            .with_context(|| format!("Couldn't parse content-length as a number: {:?}", self.headers().get(&header_name)))?;
        let mut buf = vec![0u8; content_length];
        self.reader.read_exact(&mut buf).await?;
        let res = String::from_utf8(buf)?;
        Ok(res)
    }

    pub async fn respond(self, response: Response) -> Result<()> {
        let mut writer = tokio::io::BufWriter::new(self.reader.into_inner());
        let http_response = 
        match response {
            Response::Text{text, headers} => {
                let mut builder = HttpResponseBuilder::new();
                builder.body(&text);
                builder.content_text();
                for header in headers {
                    builder.header(header);
                }
                builder.build()
            },
            Response::Html { content, headers } => {
                let mut builder = HttpResponseBuilder::new();
                builder.body(&content);
                builder.content_html();
                for header in headers {
                    builder.header(header);
                };
                builder.build()
            },
            Response::Json { content, headers } => {
                let mut builder = HttpResponseBuilder::new();
                builder.body(&content);
                builder.content_json();
                for header in headers {
                    builder.header(header);
                };
                builder.build()
            },
            Response::Redirect { location, headers } => {
                let mut builder = HttpResponseBuilder::new();
                builder.status(HttpStatusCode::SeeOther);
                builder.header((CaseInsensitiveString::from("Location"), location));
                for header in headers {
                    builder.header(header);
                }
                builder.build()
            },
            Response::EventSource { retry, mut stream } => {
                tokio::spawn(async move {
                    if let Err(e) = handle_event_stream(writer.into_inner(), retry, &mut stream).await {
                        // TODO differentiate between client disconnection and other errors

                        // Client disconnected, close drain and drop the stream
                        log_internal_error(e);

                        stream.close();
                        while stream.recv().await.is_some() {};
                        drop(stream);
                    };
                });
                return Ok(());
            },
            Response::BadRequest => {
                HttpResponseBuilder::new()
                    .status(HttpStatusCode::BadRequest)
                    .body("Bad request")
                    .build()
            },
            Response::InternalServerError => {
                HttpResponseBuilder::new()
                    .status(HttpStatusCode::InternalServerError)
                    .body("Internal Server Error")
                    .build()
            },
            Response::Empty => {
                HttpResponseBuilder::new().build()
            }
        };
        writer.write_all(&http_response.into_bytes()).await?;
        writer.shutdown().await?;
        Ok(())
    }
}

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
        stream: tokio::sync::mpsc::UnboundedReceiver<EventSourceEvent>,
    },
    BadRequest,
    InternalServerError,
    Empty,
}

#[derive(Debug)]
pub struct EventSourceEvent {
    pub data: String,
    pub id: String,
    pub event: Option<String>,
}

pub trait RequestHandler: 'static + Send + Clone {
    type Error: std::error::Error;
    fn handle(self, request: &mut Request) -> impl std::future::Future<Output = Result<Response, Self::Error>> + Send;
}

pub async fn run_server(addr: &str, request_handler: impl RequestHandler, cancellation_token: CancellationToken) -> Result<()> {

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
                    utils::log_internal_error(e);
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
                },
            };
        
            let response = match request_handler.handle(&mut request).await {
                Ok(response) => response,
                Err(e) => {
                    utils::log_internal_error(e);
                    return
                }, 
            };
        
            if let Err(e) = request.respond(response).await {
                utils::log_internal_error(e)
            };
        });
    };
    eprintln!("Shutting down server...Success");
    Ok(())
}

async fn handle_event_stream(mut tcp_stream: TcpStream, retry: Option<i32>, event_stream: &mut UnboundedReceiver<EventSourceEvent>) -> Result<()> {
    let http_response = HttpResponseBuilder::new()
        .content_event_stream()
        .build();

    tcp_stream.write_all(&http_response.into_bytes()).await?;

    if let Some(retry_value) = retry {
        tcp_stream.write_all(&format!("retry: {retry_value}\n").as_bytes()).await?;
    };

    tcp_stream.flush().await?;
    loop {
        // tokio::select! {
        //     ready_res = tcp_stream.ready(Interest::ERROR | !Interest::WRITABLE) => {
        //         eprintln!("Error state: {ready_res:?}");
        //     },
        //     new_event = event_stream.recv() => {
        //         eprintln!("Received event: {new_event:?}")
        //     },
        // }

        // let ready = tcp_stream.ready(Interest::WRITABLE | Interest::ERROR).await?;

        // let loop_id = uuid::Uuid::new_v4(); 
        // if ready.is_error() {
        //     eprintln!("{loop_id}: Error state");

        //     event_stream.close();
        //     while event_stream.recv().await.is_some() {};
        //     bail!("TCP stream error");
        // } else if ready.is_writable() {
        //     eprintln!("{loop_id}: Writable state");
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                tcp_stream.write_all(b": keep-alive\n").await.context("Keep alive failed")?;
            },
            next_event = event_stream.recv() => {
                match next_event {
                    Some(event) => {
                        send_event_to_event_source_stream(&mut tcp_stream, event).await.context("Send event failed")?;
                    },
                    None => {
                        break
                    },
                }
            },
        }
            
    }
    // };

    Ok(())
}

async fn send_event_to_event_source_stream(tcp_stream: &mut TcpStream, event: EventSourceEvent) -> Result<()> {
    // let loop_id = uuid::Uuid::new_v4();
    // eprintln!("{loop_id}: Received event");
    let mut response_str = String::new();
    if let Some(event_type) = event.event {
        response_str.push_str(&format!("event: {}\n", event_type));
    }
    for line in event.data.lines() {
        response_str.push_str(&format!("data: {line}\n"))
    };
    response_str.push_str(&format!("id: {}\n", event.id));
    response_str.push_str("\n");

    tcp_stream.write_all(response_str.as_bytes()).await?;
    tcp_stream.flush().await?;
    // eprintln!("{loop_id}: Sent event");
    Ok(())
}