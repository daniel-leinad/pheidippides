use anyhow::{bail, Result};
use std::collections::HashMap;
use super::http::{Request, Header};

pub fn log_internal_error(error: impl std::fmt::Display) {
    eprintln!("SERVER ERROR: {:#}", error);
}

pub fn get_headers_hashmap(request: &Request) -> HashMap<String, String> {
    let headers = {
        let mut res: HashMap<String, String> = HashMap::new();
        for header in request.headers() {
            res.insert(
                header.field.as_str().as_str().into(),
                header.value.clone().into(),
            );
        }
        res
    };
    headers
}

pub enum CookieParsingError {
    IncorrectHeader,
}

pub fn get_cookies_hashmap(
    headers: HashMap<String, String>,
) -> Result<HashMap<String, String>, CookieParsingError> {
    let mut res = HashMap::new();
    if let Some(cookie_list) = headers.get("Cookie") {
        for cookie in cookie_list.split("; ") {
            let (key, value) = match cookie.split_once('=') {
                Some(key_value) => key_value,
                None => return Err(CookieParsingError::IncorrectHeader),
            };
            res.insert(key.into(), value.into());
        }
    }
    Ok(res)
}

pub fn header_set_cookie(key: &str, value: &str) -> Result<Header> {
    match Header::from_bytes("Set-Cookie", format!("{key}={value}")) {
        Ok(header) => Ok(header),
        Err(()) => bail!("Couldn'r create header Set-Cookie {key}={value}"),
    }
}