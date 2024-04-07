#![feature(slice_split_once)]
#![feature(iter_intersperse)]

mod authorization;
mod db;
mod http;
mod serde_form_data;
mod utils;
mod sessions;
mod routing;
mod fs;

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    host: String,
    #[arg(short, long)]
    port: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let host = args.host;
    let port = args.port;
    let addr = format!("{host}:{port}");

    // let db_access = db::mock::Db::new().await;
    let db_access = db::pg::Db::new("postgres://postgres:12345@localhost/pheidippides_test_1")?;

    let request_handler = routing::RequestHandler::new(db_access);

    http::run_server(&addr, request_handler).await.with_context(|| format!("Unable to start server at {addr}"))?;

    Ok(())
}