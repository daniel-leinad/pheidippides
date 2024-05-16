use anyhow::{Context, Result};
use clap::Parser;
use tokio_util::sync::CancellationToken;

use pheidippides_messenger::data_access::DataAccess;
use pheidippides_web::request_handler;

use mock_db;
use pheidippides_messenger::authorization::AuthStorage;
use postgres_db;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    host: String,
    #[arg(short, long)]
    port: u32,
    #[arg(long, id="CONNECTION URL", help="Database conneciton url. Format: postgresql://[user[:password]@][host][:port][/dbname][?param1=value1&...]")]
    db: Option<String>,
    #[arg(long)]
    mock: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let host = args.host;
    let port = args.port;
    let addr = format!("{host}:{port}");

    let cancellation_token = make_cancellation_token();

    let use_mock = args.mock;

    if use_mock {
        let db_access = mock_db::Db::new().await;
        run_server(db_access, &addr, cancellation_token).await?;
    } else {
        let db_connection = args.db.context("Database connection url must be specified")?;
        let db_access = postgres_db::Db::new(&db_connection).await?;
        db_access.check_migrations().await?;
        let db_graceful_shutdown = db_access.graceful_shutdown(cancellation_token.clone());

        run_server(db_access, &addr, cancellation_token).await?;

        db_graceful_shutdown.await.context("Join error in thread handling database connection shutdown")?;
    }
    
    Ok(())
}

async fn run_server<T: DataAccess + AuthStorage>(data_access: T, addr: &str, cancellation_token: CancellationToken) -> Result<()> {
    let request_handler = request_handler::RequestHandler::new(data_access.clone(), data_access);
    web_server::run_server(addr, request_handler, cancellation_token.clone()).await.with_context(|| format!("Unable to start server at {}", addr))?;
    Ok(())
}

fn make_cancellation_token() -> CancellationToken {
    let cancellation_token = CancellationToken::new();

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