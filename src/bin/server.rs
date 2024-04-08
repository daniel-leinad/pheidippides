use anyhow::{Context, Result};
use clap::Parser;

use pheidippides::{db, routing, http};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    host: String,
    #[arg(short, long)]
    port: u32,
    #[arg(long, id="CONNECTION URL", help="Database conneciton url. Format: postgresql://[user[:password]@][host][:port][/dbname][?param1=value1&...]")]
    db: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let host = args.host;
    let port = args.port;
    let addr = format!("{host}:{port}");
    let db_connection = args.db;

    let cancellation_token = make_cancellation_token();

    let db_access = db::pg::Db::new(&db_connection).await?;
    db_access.check_migrations().await?;
    let db_graceful_shutdown = db_access.graceful_shutdown(cancellation_token.clone());

    let request_handler = routing::RequestHandler::new(db_access.clone());

    http::run_server(&addr, request_handler, cancellation_token.clone()).await.with_context(|| format!("Unable to start server at {addr}"))?;
    
    db_graceful_shutdown.await.context("Join error in thread handling database connection shutdown")?;

    Ok(())
}


fn make_cancellation_token() -> tokio_util::sync::CancellationToken {
    let cancellation_token = tokio_util::sync::CancellationToken::new();

    let cloned_token = cancellation_token.clone();
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                eprintln!("Received shutdown signal");
            },
            Err(err) => {
                eprintln!("Unable to listen for shutdown signal: {}", err);
            },
        };
        cloned_token.cancel();
    });

    cancellation_token
}