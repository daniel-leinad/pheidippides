use std::collections::HashMap;
use crate::utils::CaseInsensitiveString;

pub type Header = (CaseInsensitiveString, String);

pub enum CookieParsingError {
    IncorrectHeader,
}

pub fn get_cookies_hashmap(
    headers: &HashMap<CaseInsensitiveString, String>,
) -> anyhow::Result<HashMap<String, String>, CookieParsingError> {
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
