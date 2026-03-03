use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use sqlx::postgres::PgPoolOptions;
use dotenvy::dotenv;
use dashmap::DashMap;
use serde::{Serialize, Deserialize};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tokio::signal;

mod auth;
mod config;
mod crypto;
mod db;
mod db_setup;
mod error;
mod logging;
mod positions;
mod stocks;

#[derive(Clone, Serialize, Deserialize)]
pub struct StockQuote {
    pub price: f64,
    pub change: f64,
    pub change_percent: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Shared application state, accessible via Axum's `State` extractor.
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: config::AppConfig,
    pub encryption_key: [u8; 32],
    pub price_cache: DashMap<String, StockQuote>,
    pub http_client: reqwest::Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    // Initialize industry-level structured logging
    let _log_guard = logging::init_tracing();
    tracing::info!("Starting Stock Analysis Platform Backend...");

    // Load typed configuration from environment
    let app_config = config::AppConfig::from_env()?;
    tracing::info!("Configuration loaded successfully");

    // Parse encryption key (supports hex or legacy UTF-8 fallback)
    let encryption_key = app_config.parse_encryption_key()?;

    // Ensure database exists (Pre-run check)
    if let Err(e) = db_setup::ensure_db_exists().await {
        tracing::warn!("Pre-run database check failed: {}. Attempting to proceed...", e);
    }

    // Database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(app_config.server.max_db_connections)
        .connect(&app_config.database_url)
        .await?;

    // Run migrations
    sqlx::migrate!().run(&pool).await?;

    // Shared HTTP client with connection pooling
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let state = Arc::new(AppState {
        db: pool,
        config: app_config.clone(),
        encryption_key,
        price_cache: DashMap::new(),
        http_client,
    });

    // Background worker for real-time stock refresh
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

    // Build application with routes, State extractor, and tracing middleware
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/stocks/search", get(stocks::search))
        .route("/watchlist", get(stocks::get_watchlist).post(stocks::add_to_watchlist))
        .route("/watchlist/:symbol", axum::routing::delete(stocks::remove_from_watchlist))
        .route("/positions", get(positions::get_positions).post(positions::add_position))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    // Start server with graceful shutdown
    let addr = SocketAddr::new(
        app_config.server.host.parse()?,
        app_config.server.port,
    );
    tracing::info!("Server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shut down gracefully");
    Ok(())
}

/// Enhanced health check that verifies database connectivity.
async fn health_check(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::Json;

    let db_ok = sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .is_ok();

    let cache_size = state.price_cache.len();

    if db_ok {
        (StatusCode::OK, Json(serde_json::json!({
            "status": "healthy",
            "database": "connected",
            "cache_entries": cache_size
        }))).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
            "status": "unhealthy",
            "database": "disconnected",
            "cache_entries": cache_size
        }))).into_response()
    }
}

/// Listens for Ctrl+C or SIGTERM to trigger graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { tracing::info!("Received Ctrl+C, shutting down..."); },
        _ = terminate => { tracing::info!("Received SIGTERM, shutting down..."); },
    }
}
