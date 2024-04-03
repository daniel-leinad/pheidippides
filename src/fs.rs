use std::fs;
use anyhow::{Context, Result};

pub fn load_template(file_name: &str) -> Result<Vec<u8>> {
    fs::read(file_name).with_context(|| format!("Couldn't load template file {file_name}"))
}

pub fn load_template_as_string(file_name: &str) -> Result<String> {
    let bytes = load_template(file_name)?;
    String::from_utf8(bytes).with_context(|| format!("Invalid utf-8 in template {file_name}"))
}