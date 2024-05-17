use crate::http_response::{HttpResponseBuilder, HttpStatusCode};
use crate::method::Method;
use crate::response::Response;
use anyhow::Context;
use pheidippides_utils::utils::{log_internal_error, CaseInsensitiveString};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;

pub struct Request<T> {
    reader: BufReader<T>,
    method: Method,
    url: String,
    headers: HashMap<CaseInsensitiveString, String>,
}

impl Request<TcpStream> {
    pub async fn respond(self, response: Response) -> anyhow::Result<()> {
        let mut writer = BufWriter::new(self.reader.into_inner());
        let http_response = match response {
            Response::Text { text, headers } => {
                let mut builder = HttpResponseBuilder::new();
                builder.body(&text);
                builder.content_text();
                for header in headers {
                    builder.header(header);
                }
                builder.build()
            }
            Response::Html { content, headers } => {
                let mut builder = HttpResponseBuilder::new();
                builder.body(&content);
                builder.content_html();
                for header in headers {
                    builder.header(header);
                }
                builder.build()
            }
            Response::Json { content, headers } => {
                let mut builder = HttpResponseBuilder::new();
                builder.body(&content);
                builder.content_json();
                for header in headers {
                    builder.header(header);
                }
                builder.build()
            }
            Response::Redirect { location, headers } => {
                let mut builder = HttpResponseBuilder::new();
                builder.status(HttpStatusCode::SeeOther);
                builder.header((CaseInsensitiveString::from("Location"), location));
                for header in headers {
                    builder.header(header);
                }
                builder.build()
            }
            Response::EventSource { retry, mut stream } => {
                tokio::spawn(async move {
                    if let Err(e) = crate::event_source::handle_event_stream(
                        writer.into_inner(),
                        retry,
                        &mut stream,
                    )
                    .await
                    {
                        log_internal_error(e);
                    };

                    // close drain and drop the subscription
                    stream.close();
                    while stream.recv().await.is_some() {}
                });
                return Ok(());
            }
            Response::BadRequest => HttpResponseBuilder::new()
                .status(HttpStatusCode::BadRequest)
                .body("Bad request")
                .build(),
            Response::InternalServerError => HttpResponseBuilder::new()
                .status(HttpStatusCode::InternalServerError)
                .body("Internal Server Error")
                .build(),
            Response::Empty => HttpResponseBuilder::new().build(),
        };
        writer.write_all(&http_response.into_bytes()).await?;
        writer.shutdown().await?;
        Ok(())
    }
}

impl<T: AsyncRead + Unpin> Request<T> {
    pub async fn try_from_stream(stream: T) -> anyhow::Result<Self> {
        let mut reader = BufReader::new(stream);

        let mut first_line = String::new();
        reader
            .read_line(&mut first_line)
            .await
            .context("Could not read line")?;
        let mut first_line_split = first_line.split_whitespace();
        let context = || format!("Could not parse first line: {first_line}");
        let method: Method = first_line_split.next().with_context(context)?.parse()?;
        let url = first_line_split.next().with_context(context)?.to_owned();

        let mut headers = HashMap::new();
        loop {
            let mut next_line = String::new();
            reader
                .read_line(&mut next_line)
                .await
                .context("Could not read line")?;
            if next_line == "\r\n" {
                break;
            } else {
                let (key, value) = next_line
                    .split_once(": ")
                    .with_context(|| format!("Incorrect header: {next_line}"))?;
                headers.insert(key.into(), value.trim_end().to_string());
            }
        }

        Ok(Request {
            reader,
            method,
            url,
            headers,
        })
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

    pub async fn content(&mut self) -> anyhow::Result<String> {
        let header_name: CaseInsensitiveString = "content-length".into();
        let content_length: usize = self
            .headers()
            .get(&header_name)
            .context("Content-Length header is missing")?
            .parse()
            .with_context(|| {
                format!(
                    "Couldn't parse content-length as a number: {:?}",
                    self.headers().get(&header_name)
                )
            })?;
        let mut buf = vec![0u8; content_length];
        self.reader.read_exact(&mut buf).await?;
        let res = String::from_utf8(buf)?;
        Ok(res)
    }
}
