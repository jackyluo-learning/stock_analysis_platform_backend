use axum::{
    routing::{get, post},
    Router, Extension,
};
use std::net::SocketAddr;
use std::sync::Arc;
use sqlx::postgres::PgPoolOptions;
use dotenvy::dotenv;
use dashmap::DashMap;
use serde::{Serialize, Deserialize};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

mod auth;
mod stocks;
mod db;
mod crypto;
mod positions;
mod db_setup;
mod logging;

#[derive(Clone, Serialize, Deserialize)]
pub struct StockQuote {
    pub price: f64,
    pub change: f64,
    pub change_percent: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

pub struct AppState {
    pub db: sqlx::PgPool,
    pub jwt_secret: String,
    pub encryption_key: [u8; 32],
    pub price_cache: DashMap<String, StockQuote>,
    pub alpaca_api_key: String,
    pub alpaca_api_secret: String,
    pub finnhub_api_key: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    // Initialize industry-level structured logging
    // The guard must be kept alive to ensure logs are flushed
    let _log_guard = logging::init_tracing();
    tracing::info!("Starting Stock Analysis Platform Backend...");

    // Ensure database exists (Pre-run check)
    if let Err(e) = db_setup::ensure_db_exists().await {
        tracing::warn!("Pre-run database check failed: {}. Attempting to proceed...", e);
    }

    // Database connection pool
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!().run(&pool).await?;

    // JWT Secret and Encryption Key from env
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let enc_key_str = std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set");
    let mut encryption_key = [0u8; 32];
    let key_bytes = enc_key_str.as_bytes();
    for i in 0..32.min(key_bytes.len()) {
        encryption_key[i] = key_bytes[i];
    }

    // API keys for market data providers
    let alpaca_api_key = std::env::var("ALPACA_API_KEY").expect("ALPACA_API_KEY must be set");
    let alpaca_api_secret = std::env::var("ALPACA_API_SECRET").expect("ALPACA_API_SECRET must be set");
    let finnhub_api_key = std::env::var("FINNHUB_API_KEY").expect("FINNHUB_API_KEY must be set");

    let state = Arc::new(AppState {
        db: pool,
        jwt_secret,
        encryption_key,
        price_cache: DashMap::new(),
        alpaca_api_key,
        alpaca_api_secret,
        finnhub_api_key,
    });

    // Background worker for real-time stock refresh (US.6)
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        if let Err(e) = stocks::realtime_worker(state_clone).await {
            tracing::error!("Real-time worker error: {}", e);
        }
    });

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build our application with routes and industry-standard tracing middleware
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/stocks/search", get(stocks::search))
        .route("/watchlist", get(stocks::get_watchlist).post(stocks::add_to_watchlist))
        .route("/watchlist/:symbol", axum::routing::delete(stocks::remove_from_watchlist))
        .route("/positions", get(positions::get_positions).post(positions::add_position))
        .layer(TraceLayer::new_for_http()) // Detailed request/response tracing
        .layer(cors) // Add CORS layer
        .layer(Extension(state));

    // Run it
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
