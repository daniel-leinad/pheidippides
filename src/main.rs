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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let host = args.host;
    let port = args.port;
    let addr = format!("{host}:{port}");

    let db_access = db::mock::Db::new();

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    eprintln!("Started a server at {addr}");


    loop {
        let (stream, _) = listener.accept().await?;
        let db_access = db_access.clone();
        tokio::spawn(async move {
            let mut request = match http::Request::try_from_stream(stream).await {
                Ok(req) => req,
                Err(e) => {
                    utils::log_internal_error(e);
                    return;
                },
            };
        
            let response = match routing::handle_request(&mut request, db_access).await {
                Ok(response) => response,
                Err(e) => {
                    utils::log_internal_error(e);
                    return
                }, 
            };
        
            if let Err(e) = request.respond(response).await {
                utils::log_internal_error(e)
            };
        });
    };
}