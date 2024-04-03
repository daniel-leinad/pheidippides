#![feature(slice_split_once)]
#![feature(iter_intersperse)]

mod authorization;
mod db;
mod http;
mod serde_form_data;
mod utils;
mod sessions;
mod routing;

use anyhow::{bail, Context, Result};
use clap::Parser;
use db::{MessageId, UserId};
use http::{Header, Request, Response, Server};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::RwLock;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
};
use utils::{log_internal_error, get_cookies_hashmap, get_headers_hashmap, header_set_cookie};

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


