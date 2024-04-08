mod http_response;

use std::{collections::HashMap, str::FromStr};

use anyhow::{Result, Context};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, net::TcpStream};
use tokio_util::sync::CancellationToken;

use crate::utils::{self, CaseInsensitiveString};
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
    BadRequest,
    InternalServerError,
    Empty,
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
                Err(e) => {
                    utils::log_internal_error(e);
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