use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq)]
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
            _ => Err(MethodParseError::IncorrectMethod),
        }
    }
}
