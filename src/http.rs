use std::{collections::HashMap, str::FromStr};

use anyhow::{Result, Context};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, net::TcpStream};

use crate::utils::CaseInsensitiveString;

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
        match response {
            Response::Text(text) => {
                let text = text.as_bytes();
                writer.write(b"HTTP/1.1 200 OK\r\n").await?;
                writer.write(b"Content-Type: text/html\r\n").await?;
                writer.write(&format!("Content-Length: {}\r\n", text.len()).as_bytes()).await?;
                writer.write(b"\r\n").await?;
                writer.write(text).await?;
                writer.flush().await?;
                Ok(())
            },
            Response::HtmlPage { bytes, headers } => {
                writer.write(b"HTTP/1.1 200 OK\r\n").await?;
                writer.write(&format!("Content-Length: {}\r\n", bytes.len()).as_bytes()).await?;
                for (key, value) in headers {
                    writer.write(&format!("{key}: {value}\r\n").as_bytes()).await?;
                };
                writer.write(b"\r\n").await?;
                writer.write(&bytes).await?;
                writer.flush().await?;
                Ok(())
            },
            Response::Redirect { location, headers } => {
                writer.write(b"HTTP/1.1 303 See Other\r\n").await?;
                writer.write(&format!("Location: {location}\r\n").as_bytes()).await?;
                for (key, value) in headers {
                    writer.write(&format!("{key}: {value}\r\n").as_bytes()).await?;
                };
                writer.write(b"\r\n").await?;
                writer.flush().await?;
                Ok(())
            },
            Response::BadRequest => {
                let content = b"Bad request";
                writer.write(b"HTTP/1.1 400 Bad Request\r\n").await?;
                writer.write(&format!("Content-Length: {}\r\n", content.len()).as_bytes()).await?;
                writer.write(b"\r\n").await?;
                writer.write(content).await?;
                writer.flush().await?;
                Ok(())
            },
            Response::InternalServerError => {
                let content = b"Internal Server Error";
                writer.write(b"HTTP/1.1 500 Internal Server Error\r\n").await?;
                writer.write(&format!("Content-Length: {}\r\n", content.len()).as_bytes()).await?;
                writer.write(b"\r\n").await?;
                writer.write(content).await?;
                writer.flush().await?;
                Ok(())
            },
            Response::Empty => {
                writer.write(b"HTTP/1.1 200 OK\r\n").await?;
                writer.write(b"\r\n").await?;
                writer.flush().await?;
                Ok(())
            }
        }
    }
}

pub enum Response {
    HtmlPage {
        bytes: Vec<u8>,
        headers: Vec<Header>,
    },
    Text(String),
    Redirect{
        location: String, 
        headers: Vec<Header>
    },
    BadRequest,
    InternalServerError,
    Empty,
}

// pub async fn run_server<F>(addr: &str, mut request_handler: F) -> Result<()>
// where
//     F: Fn(&mut Request) -> Result<Response>,
//     F: Sync + Clone + 'static,
//  {
//     let listener = tokio::net::TcpListener::bind(addr).await?;
//     eprintln!("Started a server at {addr}");

//     // let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

//     // let request_handler = Arc::new(Mutex::new(request_handler));
//     loop {
//         let (mut stream, _) = listener.accept().await?;
//         tokio::spawn(proccess_stream(stream, &request_handler.clone())); 
//     }
// }

// async fn proccess_stream(stream: TcpStream, request_handler: &impl Fn(&mut Request) -> Result<Response>) {
//     let mut request = match Request::try_from_stream(stream).await {
//         Ok(req) => req,
//         Err(e) => {
//             log_internal_error(e);
//             return;
//         },
//     };

//     // let request = Arc::new(Mutex::new(request));

//     let response = match request_handler(&mut request) {
//         Ok(response) => response,
//         Err(e) => {
//             log_internal_error(e);
//             return
//         }, 
//     };

//     if let Err(e) = request.respond(response).await {
//         log_internal_error(e)
//     };
// }