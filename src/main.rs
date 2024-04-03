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

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    host: String,
    #[arg(short, long)]
    port: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let host = args.host;
    let port = args.port;
    let addr = format!("{host}:{port}");

    let db_access = db::mock::Db::new();

    let request_handler = |request| {
        routing::handle_request(request, db_access.clone())
    };

    http::run_server(&addr, request_handler)
}


