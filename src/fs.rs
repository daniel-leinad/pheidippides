use std::path::Path;
use anyhow::{Context, Result};

pub async fn load_template(file_name: &str) -> Result<Vec<u8>> {
    let path = Path::new("templates").join(file_name);
    tokio::fs::read(path).await.with_context(|| format!("Couldn't load template file {file_name}"))
}

pub async fn load_template_as_string(file_name: &str) -> Result<String> {
    let bytes = load_template(file_name).await?;
    String::from_utf8(bytes).with_context(|| format!("Invalid utf-8 in template {file_name}"))
}