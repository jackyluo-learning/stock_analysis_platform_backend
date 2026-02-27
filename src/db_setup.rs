use sqlx::postgres::PgConnection;
use sqlx::{Connection, Executor};
use dotenvy::dotenv;

pub async fn ensure_db_exists() -> anyhow::Result<()> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
    
    // Parse URL to get connection to 'postgres'
    let url_parts: Vec<&str> = database_url.split('@').collect();
    if url_parts.len() < 2 {
        return Err(anyhow::anyhow!("Invalid DATABASE_URL format"));
    }
    
    let host_db_part = url_parts[1]; 
    let host_db_split: Vec<&str> = host_db_part.split('/').collect();
    let host = host_db_split[0]; 
    let db_query = host_db_split.get(1).unwrap_or(&""); 
    let query_part = if let Some(pos) = db_query.find('?') {
        &db_query[pos..] 
    } else {
        ""
    };

    let postgres_url = format!("{}@{}/postgres{}", url_parts[0], host, query_part);
    
    tracing::info!("Pre-run check: Connecting to PostgreSQL server at {}...", host);
    let mut conn = PgConnection::connect(&postgres_url).await?;

    let row: (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'stock_platform')")
        .fetch_one(&mut conn)
        .await?;

    if !row.0 {
        tracing::info!("Pre-run check: Database 'stock_platform' does not exist. Creating it...");
        conn.execute("CREATE DATABASE stock_platform").await?;
        tracing::info!("Pre-run check: Database 'stock_platform' created.");
    } else {
        tracing::info!("Pre-run check: Database 'stock_platform' already exists.");
    }

    Ok(())
}
