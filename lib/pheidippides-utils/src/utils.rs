use std::hash::Hash;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CaseInsensitiveString(String);

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
