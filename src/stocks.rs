use axum::{
    extract::{Json, Query, Path},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{AppState, auth::AuthUser, StockQuote};
use tokio::time::{interval, Duration};
use dashmap::DashMap;

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
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    // Use Finnhub symbol search API
    let url = format!(
        "https://finnhub.io/api/v1/search?q={}&token={}",
        params.q, state.finnhub_api_key
    );
    let client = reqwest::Client::new();

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to contact Finnhub: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to contact Finnhub").into_response();
        }
    };

    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to read Finnhub response body: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read Finnhub response").into_response();
        }
    };

    match parse_finnhub_search_response(&text) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => {
            tracing::error!("Failed to parse Finnhub response: {}. Body: {}", e, text);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to parse Finnhub response").into_response()
        }
    }
}

pub fn parse_finnhub_search_response(text: &str) -> anyhow::Result<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(text).map_err(|e| anyhow::anyhow!(e))
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
    let mut timer = interval(Duration::from_secs(10));
    let client = reqwest::Client::new();

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

        if symbols.is_empty() {
            continue;
        }

        // Build comma-separated symbol list for Alpaca batch snapshot
        let symbol_list: Vec<&str> = symbols.iter().map(|r| r.symbol.as_str()).collect();
        let symbols_param = symbol_list.join(",");

        let url = format!(
            "https://data.alpaca.markets/v2/stocks/snapshots?symbols={}&feed=iex",
            symbols_param
        );

        let resp = match client
            .get(&url)
            .header("APCA-API-KEY-ID", &state.alpaca_api_key)
            .header("APCA-API-SECRET-KEY", &state.alpaca_api_secret)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Worker failed to contact Alpaca: {}", e);
                continue;
            }
        };

        match resp.json::<serde_json::Value>().await {
            Ok(json) => {
                process_alpaca_snapshots(&state.price_cache, json);
            }
            Err(e) => tracing::error!("Worker failed to parse Alpaca response: {}", e),
        }
    }
}

pub fn process_alpaca_snapshots(price_cache: &DashMap<String, StockQuote>, json: serde_json::Value) {
    // Alpaca snapshot response is a map: { "AAPL": { ... }, "MSFT": { ... } }
    if let Some(obj) = json.as_object() {
        for (symbol, snapshot) in obj {
            // latestTrade.p = latest trade price
            let price = snapshot["latestTrade"]["p"].as_f64();
            // dailyBar.o = today's open, prevDailyBar.c = previous close
            let prev_close = snapshot["prevDailyBar"]["c"].as_f64();

            if let Some(p) = price {
                let (change, change_percent) = if let Some(pc) = prev_close {
                    let c = p - pc;
                    let cp = if pc != 0.0 { (c / pc) * 100.0 } else { 0.0 };
                    (c, cp)
                } else {
                    (0.0, 0.0)
                };

                price_cache.insert(symbol.clone(), StockQuote {
                    price: p,
                    change,
                    change_percent,
                    last_updated: chrono::Utc::now(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashmap::DashMap;
    use serde_json::json;

    #[test]
    fn test_process_alpaca_snapshots() {
        let price_cache = DashMap::new();
        let json = json!({
            "AAPL": {
                "latestTrade": { "p": 150.0 },
                "prevDailyBar": { "c": 145.0 }
            },
            "MSFT": {
                "latestTrade": { "p": 300.0 },
                "prevDailyBar": { "c": 310.0 }
            }
        });

        process_alpaca_snapshots(&price_cache, json);

        assert_eq!(price_cache.len(), 2);
        
        let aapl = price_cache.get("AAPL").unwrap();
        assert_eq!(aapl.price, 150.0);
        assert_eq!(aapl.change, 5.0);
        assert!((aapl.change_percent - 3.448).abs() < 0.001);

        let msft = price_cache.get("MSFT").unwrap();
        assert_eq!(msft.price, 300.0);
        assert_eq!(msft.change, -10.0);
        assert!((msft.change_percent - (-3.225)).abs() < 0.001);
    }

    #[test]
    fn test_parse_finnhub_search_response() {
        let body = r#"{"count":1,"result":[{"description":"APPLE INC","displaySymbol":"AAPL","symbol":"AAPL","type":"Common Stock"}]}"#;
        let result = parse_finnhub_search_response(body).unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["result"][0]["symbol"], "AAPL");
    }
}
