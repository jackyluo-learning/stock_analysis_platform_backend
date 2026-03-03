use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{AppState, auth::AuthUser, crypto};
use crate::error::AppError;
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
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(payload): Json<AddPositionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let details_json = serde_json::to_vec(&payload.details)?;

    let encrypted_details = crypto::encrypt(&details_json, &state.encryption_key)
        .map_err(|e| AppError::Crypto(e.to_string()))?;

    sqlx::query(
        "INSERT INTO positions (user_id, symbol, shares, cost_basis, encrypted_details) VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(user.id)
    .bind(&payload.symbol)
    .bind(&payload.shares)
    .bind(&payload.cost_basis)
    .bind(&encrypted_details)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::CREATED)
}

pub async fn get_positions(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let rows = sqlx::query_as::<_, PositionRow>(
        "SELECT id, symbol, shares, cost_basis, encrypted_details FROM positions WHERE user_id = $1"
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let response: Vec<PositionResponse> = rows
        .into_iter()
        .map(|row| {
            let decrypted_details = row.encrypted_details
                .as_ref()
                .and_then(|enc| crypto::decrypt(enc, &state.encryption_key).ok())
                .and_then(|v| serde_json::from_slice::<PositionDetails>(&v).ok())
                .unwrap_or(PositionDetails {
                    broker: String::new(),
                    notes: String::new(),
                });

            PositionResponse {
                id: row.id,
                symbol: row.symbol,
                shares: row.shares,
                cost_basis: row.cost_basis,
                details: decrypted_details,
            }
        })
        .collect();

    Ok((StatusCode::OK, Json(response)))
}

#[derive(sqlx::FromRow)]
struct PositionRow {
    id: Uuid,
    symbol: String,
    shares: BigDecimal,
    cost_basis: BigDecimal,
    encrypted_details: Option<Vec<u8>>,
}
