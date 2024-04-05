use anyhow::Result;
use serde::Serializer;
use std::collections::HashMap;
use std::hash::Hash;
use super::http::Header;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CaseInsensitiveString(String);

pub fn serialize_uuid<S: Serializer>(uuid: &uuid::Uuid, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&uuid.to_string())
}

impl From<&str> for CaseInsensitiveString {
    fn from(value: &str) -> Self {
        Self(value.to_lowercase())
    }
}

impl std::fmt::Display for CaseInsensitiveString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn log_internal_error(error: impl std::fmt::Display) {
    eprintln!("SERVER ERROR: {:#}", error);
}

pub enum CookieParsingError {
    IncorrectHeader,
}

pub fn get_cookies_hashmap(
    headers: &HashMap<CaseInsensitiveString, String>,
) -> Result<HashMap<String, String>, CookieParsingError> {
    let mut res = HashMap::new();
    if let Some(cookie_list) = headers.get(&"Cookie".into()) {
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

pub fn header_set_cookie(key: &str, value: &str) -> Header {
    ("Set-Cookie".into(), format!("{key}={value}"))
}