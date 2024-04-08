use anyhow::{Context, Result};
use clap::Parser;

use pheidippides::{db, routing, http};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    host: String,
    #[arg(short, long)]
    port: u8,
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

    let db_access = db::pg::Db::new(&db_connection).await?;
    db_access.check_migrations().await?;

    let request_handler = routing::RequestHandler::new(db_access);

    http::run_server(&addr, request_handler).await.with_context(|| format!("Unable to start server at {addr}"))?;

    Ok(())
}