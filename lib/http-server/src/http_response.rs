use std::collections::HashMap;
use pheidippides_utils::utils::CaseInsensitiveString;
use super::Header;

pub struct HttpResponse(Vec<u8>);

impl HttpResponse {
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

pub struct HttpResponseBuilder<'a> {
    version: HttpVersion,
    status: HttpStatusCode,
    headers: HashMap<CaseInsensitiveString, String>,
    body: Option<&'a str>,
}

impl<'a> HttpResponseBuilder<'a> {
    pub fn new() -> Self {
        let version = HttpVersion::Http11;
        let status = HttpStatusCode::OK;
        let headers = HashMap::new();
        let body = None;
        HttpResponseBuilder{version, status, headers, body}
    }

    pub fn build(&mut self) -> HttpResponse {
        let mut lines = vec![];
        lines.push(format!("{} {}\r\n", self.version, self.status).into_bytes());

        // headers
        if let Some(body) = self.body {
            self.headers.insert(CaseInsensitiveString::from("Content-Length"), format!("{}", body.len()));
        };
        for (key, value) in self.headers.iter() {
            lines.push(format!("{key}: {value}\r\n").into_bytes());
        };
        lines.push(b"\r\n".into());

        // body
        if let Some(body) = self.body {
            lines.push(body.as_bytes().to_owned());
        };

        let res: Vec<u8> = lines.concat();
        HttpResponse(res)
    }

    pub fn status(&mut self, status: HttpStatusCode) -> &mut Self {
        self.status = status;
        self
    }

    pub fn header(&mut self, (key, value): Header) -> &mut Self {
        self.headers.insert(key, value);
        self
    }

    pub fn body(&mut self, body: &'a str) -> &mut Self {
        self.body = Some(body);
        self
    }

    pub fn content_text(&mut self) -> &mut Self {
        self.headers.insert(CaseInsensitiveString::from("Content-Type"), "text/plain; charset=utf-8".to_owned());
        self
    }

    pub fn content_html(&mut self) -> &mut Self {
        self.headers.insert(CaseInsensitiveString::from("Content-Type"), "text/html; charset=utf-8".to_owned());
        self
    }

    pub fn content_json(&mut self) -> &mut Self {
        self.headers.insert(CaseInsensitiveString::from("Content-Type"), "application/json; charset=utf-8".to_owned());
        self
    }

    pub fn content_event_stream(&mut self) -> &mut Self {
        self.headers.insert(CaseInsensitiveString::from("Content-Type"), "text/event-stream; charset=utf-8".to_owned());
        self
    }
}

pub enum HttpStatusCode {
    OK,
    BadRequest,
    SeeOther,
    InternalServerError,
}

impl std::fmt::Display for HttpStatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str_repr = match self {
            Self::OK => "200 OK",
            Self::BadRequest => "400 Bad Request",
            Self::SeeOther => "303 See Other",
            Self::InternalServerError => "500 Internal Server Error", 
        };
        write!(f, "{str_repr}")
    }
}

pub enum HttpVersion {
    Http11,
}

impl std::fmt::Display for HttpVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str_repr = match self {
            Self::Http11 => "HTTP/1.1",
        };
        write!(f, "{str_repr}")
    }
}
