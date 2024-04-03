pub use tiny_http::{Header, Server};
use anyhow::{Result, Context, bail};

pub struct Request {
    inner: tiny_http::Request,
}

impl From<tiny_http::Request> for Request {
    fn from(inner: tiny_http::Request) -> Self {
        Request { inner }
    }
}

impl Request {
    pub fn url(&self) -> &str {
        self.inner.url()
    }

    pub fn method(&self) -> &tiny_http::Method {
        self.inner.method()
    }

    pub fn headers(&self) -> &[Header] {
        self.inner.headers()
    }

    pub fn content(&mut self) -> Result<String> {
        let method = self.method().clone();
        let url = self.url().to_owned();
        let mut res = String::new();
        self.inner.as_reader().read_to_string(&mut res).with_context(|| format!("Couldn't respond to request {method} {url}"))?;
        Ok(res)
    }

    pub fn respond(self, response: Response) -> Result<()> {
        let http_response = match response {
            Response::HtmlPage { bytes, headers } => {
                let mut http_response = tiny_http::Response::from_data(bytes);
                headers.into_iter().for_each(|header| http_response.add_header(header));
                http_response
            },
            Response::Text(s) => tiny_http::Response::from_string(s),
            Response::Redirect{ref location, headers} => {
                let location_header = match Header::from_bytes("Location", location.as_bytes()) {
                    Ok(header) => header,
                    Err(()) => bail!("Couldn't create Location header form url {location}"),
                };
                
                let mut http_response = tiny_http::Response::from_string("Redirecting...")
                    .with_status_code(303)
                    .with_header(location_header);
                headers.into_iter().for_each(|header| http_response.add_header(header));
                http_response
            },
            Response::BadRequest => tiny_http::Response::from_string("Bad request").with_status_code(400),
            Response::InternalServerError => tiny_http::Response::from_string("500 Internal Server Error").with_status_code(500),
            Response::Empty => tiny_http::Response::from_string(""),
        };

        let method = self.method().clone();
        let url = self.url().to_owned();
        self.inner
            .respond(http_response)
            .with_context(|| format!("Couldn't respond to request {method} {url}"))
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