use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{AppState, auth::AuthUser, crypto};
use bigdecimal::BigDecimal;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct PositionDetails {
    pub broker: String,
    pub notes: String,
}

#[derive(Deserialize)]
pub struct AddPositionRequest {
    pub symbol: String,
    pub shares: BigDecimal,
    pub cost_basis: BigDecimal,
    pub details: PositionDetails,
}

#[derive(Serialize)]
pub struct PositionResponse {
    pub id: Uuid,
    pub symbol: String,
    pub shares: BigDecimal,
    pub cost_basis: BigDecimal,
    pub details: PositionDetails,
}

pub async fn add_position(
    Extension(state): Extension<Arc<AppState>>,
    user: AuthUser,
    Json(payload): Json<AddPositionRequest>,
) -> impl IntoResponse {
    let details_json = match serde_json::to_vec(&payload.details) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize details").into_response(),
    };

    let encrypted_details = match crypto::encrypt(&details_json, &state.encryption_key) {
        Ok(v) => v,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Encryption failed").into_response(),
    };

    let result = sqlx::query(
        "INSERT INTO positions (user_id, symbol, shares, cost_basis, encrypted_details) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(user.id)
    .bind(payload.symbol)
    .bind(payload.shares)
    .bind(payload.cost_basis)
    .bind(encrypted_details)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(e) => {
            tracing::error!("Failed to add position: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

pub async fn get_positions(
    Extension(state): Extension<Arc<AppState>>,
    user: AuthUser,
) -> impl IntoResponse {
    let result = sqlx::query_as::<_, PositionRow>(
        "SELECT id, symbol, shares, cost_basis, encrypted_details FROM positions WHERE user_id = $1"
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(rows) => {
            let mut response = Vec::new();
            for row in rows {
                let decrypted_details = if let Some(enc) = &row.encrypted_details {
                    match crypto::decrypt(enc, &state.encryption_key) {
                        Ok(v) => serde_json::from_slice::<PositionDetails>(&v).ok(),
                        Err(_) => None,
                    }
                } else {
                    None
                };

                response.push(PositionResponse {
                    id: row.id,
                    symbol: row.symbol,
                    shares: row.shares,
                    cost_basis: row.cost_basis,
                    details: decrypted_details.unwrap_or(PositionDetails {
                        broker: "".into(),
                        notes: "".into(),
                    }),
                });
            }
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get positions: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

#[derive(sqlx::FromRow)]
struct PositionRow {
    id: Uuid,
    symbol: String,
    shares: BigDecimal,
    cost_basis: BigDecimal,
    encrypted_details: Option<Vec<u8>>,
}
