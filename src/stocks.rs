use axum::{
    extract::{Json, Query, Path},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{AppState, auth::AuthUser, StockQuote};
use yahoo_finance_api as yahoo;
use tokio::time::{interval, Duration};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Serialize)]
pub struct WatchlistResponse {
    pub symbol: String,
    pub price: Option<f64>,
    pub change: Option<f64>,
    pub change_percent: Option<f64>,
}

pub async fn search(
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    // Manually implement search via Yahoo Finance API to avoid crate version mismatches
    let url = format!("https://query2.finance.yahoo.com/v1/finance/search?q={}", params.q);
    let client = reqwest::Client::new();
    
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to contact Yahoo Finance").into_response(),
    };

    match resp.json::<serde_json::Value>().await {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to parse Yahoo response").into_response(),
    }
}

pub async fn get_watchlist(
    Extension(state): Extension<Arc<AppState>>,
    user: AuthUser,
) -> impl IntoResponse {
    let symbols = sqlx::query_as::<_, WatchlistRow>(
        "SELECT symbol FROM watchlist WHERE user_id = $1"
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await;

    match symbols {
        Ok(rows) => {
            let mut response = Vec::new();
            for row in rows {
                let quote = state.price_cache.get(&row.symbol).map(|q| q.clone());
                response.push(WatchlistResponse {
                    symbol: row.symbol,
                    price: quote.as_ref().map(|q| q.price),
                    change: quote.as_ref().map(|q| q.change),
                    change_percent: quote.as_ref().map(|q| q.change_percent),
                });
            }
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

#[derive(sqlx::FromRow)]
struct WatchlistRow {
    symbol: String,
}

#[derive(Deserialize)]
pub struct AddToWatchlistRequest {
    pub symbol: String,
}

pub async fn add_to_watchlist(
    Extension(state): Extension<Arc<AppState>>,
    user: AuthUser,
    Json(payload): Json<AddToWatchlistRequest>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "INSERT INTO watchlist (user_id, symbol) VALUES ($1, $2) ON CONFLICT DO NOTHING"
    )
    .bind(user.id)
    .bind(payload.symbol)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

pub async fn remove_from_watchlist(
    Extension(state): Extension<Arc<AppState>>,
    user: AuthUser,
    Path(symbol): Path<String>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "DELETE FROM watchlist WHERE user_id = $1 AND symbol = $2"
    )
    .bind(user.id)
    .bind(symbol)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

pub async fn realtime_worker(state: Arc<AppState>) -> anyhow::Result<()> {
    let mut timer = interval(Duration::from_secs(2));
    let connector = match yahoo::YahooConnector::new() {
        Ok(c) => c,
        Err(e) => return Err(anyhow::anyhow!("Connector error: {}", e)),
    };

    loop {
        timer.tick().await;

        let symbols = match sqlx::query_as::<_, WatchlistRow>("SELECT DISTINCT symbol FROM watchlist")
            .fetch_all(&state.db)
            .await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Database error in worker: {}", e);
                    continue;
                }
            };

        for row in symbols {
            match connector.get_latest_quotes(&row.symbol, "1m").await {
                Ok(resp) => {
                    if let Ok(quotes) = resp.quotes() {
                        if let Some(quote) = quotes.last() {
                            let price = quote.close;
                            state.price_cache.insert(row.symbol.clone(), StockQuote {
                                price,
                                change: 0.0,
                                change_percent: 0.0,
                                last_updated: chrono::Utc::now(),
                            });
                        }
                    }
                }
                Err(e) => tracing::error!("Error fetching quote for {}: {}", row.symbol, e),
            }
        }
    }
}
