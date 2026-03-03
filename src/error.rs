use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Unified application error type for consistent error handling across all handlers.
#[derive(Debug)]
pub enum AppError {
    /// Database query or connection errors
    Database(sqlx::Error),
    /// External API call failures (Alpaca, Finnhub, etc.)
    ExternalApi(String),
    /// Authentication / authorization failures
    Unauthorized(String),
    /// Request validation errors or duplicate resources
    BadRequest(String),
    /// Encryption or decryption failures
    Crypto(String),
    /// Generic internal errors
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "A database error occurred".to_string(),
                )
            }
            AppError::ExternalApi(msg) => {
                tracing::error!("External API error: {}", msg);
                (
                    StatusCode::BAD_GATEWAY,
                    "external_api_error",
                    msg.clone(),
                )
            }
            AppError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                msg.clone(),
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "bad_request",
                msg.clone(),
            ),
            AppError::Crypto(msg) => {
                tracing::error!("Crypto error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "crypto_error",
                    "An encryption error occurred".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    msg.clone(),
                )
            }
        };

        (
            status,
            Json(ErrorResponse {
                error: error_type.to_string(),
                message,
            }),
        )
            .into_response()
    }
}

// Convenient From impls for automatic `?` operator usage
impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Database(e)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::ExternalApi(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(format!("Serialization error: {}", e))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
