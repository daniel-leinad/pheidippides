use clap::Parser;
use anyhow::Result;

use pheidippides::db;

#[derive(Parser, Debug)]
struct Args {
    #[arg(id="CONNECTION URL", help="Database conneciton url. Format: postgresql://[user[:password]@][host][:port][/dbname][?param1=value1&...]")]
    db: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let db_access = db::pg::Db::new(&args.db).await?;
    db_access.migrate().await?;
    Ok(())
}