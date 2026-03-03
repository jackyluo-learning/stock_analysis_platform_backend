use axum::{
    extract::{Json, Query, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{AppState, auth::AuthUser, StockQuote};
use crate::error::AppError;
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
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    let url = format!(
        "https://finnhub.io/api/v1/search?q={}&token={}",
        params.q, state.config.finnhub.api_key
    );

    let text = state.http_client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::ExternalApi(format!("Failed to contact Finnhub: {}", e)))?
        .text()
        .await
        .map_err(|e| AppError::ExternalApi(format!("Failed to read Finnhub response: {}", e)))?;

    let json = parse_finnhub_search_response(&text)
        .map_err(|e| {
            tracing::error!("Failed to parse Finnhub response: {}. Body: {}", e, text);
            AppError::ExternalApi("Failed to parse Finnhub response".into())
        })?;

    Ok((StatusCode::OK, Json(json)))
}

pub fn parse_finnhub_search_response(text: &str) -> anyhow::Result<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(text).map_err(|e| anyhow::anyhow!(e))
}

pub async fn get_watchlist(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let rows = sqlx::query_as::<_, WatchlistRow>(
        "SELECT symbol FROM watchlist WHERE user_id = $1"
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let response: Vec<WatchlistResponse> = rows
        .into_iter()
        .map(|row| {
            let quote = state.price_cache.get(&row.symbol).map(|q| q.clone());
            WatchlistResponse {
                symbol: row.symbol,
                price: quote.as_ref().map(|q| q.price),
                change: quote.as_ref().map(|q| q.change),
                change_percent: quote.as_ref().map(|q| q.change_percent),
            }
        })
        .collect();

    Ok((StatusCode::OK, Json(response)))
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
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(payload): Json<AddToWatchlistRequest>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query(
        "INSERT INTO watchlist (user_id, symbol) VALUES ($1, $2) ON CONFLICT DO NOTHING"
    )
    .bind(user.id)
    .bind(&payload.symbol)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::CREATED)
}

pub async fn remove_from_watchlist(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    sqlx::query(
        "DELETE FROM watchlist WHERE user_id = $1 AND symbol = $2"
    )
    .bind(user.id)
    .bind(&symbol)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn realtime_worker(state: Arc<AppState>) -> anyhow::Result<()> {
    let mut timer = interval(Duration::from_secs(10));

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

        let resp = match state.http_client
            .get(&url)
            .header("APCA-API-KEY-ID", &state.config.alpaca.api_key)
            .header("APCA-API-SECRET-KEY", &state.config.alpaca.api_secret)
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
