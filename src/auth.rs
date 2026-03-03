use axum::{
    extract::{Json, State, FromRequestParts},
    http::{StatusCode, request::Parts},
    response::IntoResponse,
    async_trait,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;
use crate::error::AppError;
use uuid::Uuid;
use chrono::{Utc, Duration};

#[derive(Debug)]
pub struct AuthUser {
    pub id: Uuid,
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".into()))?;

        if !auth_header.starts_with("Bearer ") {
            return Err(AppError::Unauthorized("Invalid Authorization header format".into()));
        }

        let token = &auth_header[7..];
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.config.jwt_secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized("Invalid or expired token".into()))?;

        Ok(AuthUser { id: token_data.claims.sub })
    }
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let password_hash = hash(payload.password, DEFAULT_COST)
        .map_err(|e| AppError::Internal(format!("Error hashing password: {}", e)))?;

    sqlx::query(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3)"
    )
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&password_hash)
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.is_unique_violation() => {
            AppError::BadRequest("User already exists".into())
        }
        _ => AppError::Database(e),
    })?;

    Ok((StatusCode::CREATED, "User registered"))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user = sqlx::query_as::<_, UserRecord>(
        "SELECT id, password_hash FROM users WHERE email = $1"
    )
    .bind(&payload.email)
    .fetch_optional(&state.db)
    .await?;

    match user {
        Some(user) => {
            if verify(&payload.password, &user.password_hash).unwrap_or(false) {
                let token = generate_jwt(user.id, &state.config.jwt_secret);
                Ok(Json(AuthResponse { token }).into_response())
            } else {
                Err(AppError::Unauthorized("Invalid credentials".into()))
            }
        }
        None => Err(AppError::Unauthorized("Invalid credentials".into())),
    }
}

#[derive(sqlx::FromRow)]
struct UserRecord {
    id: Uuid,
    password_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: usize,
}

pub fn generate_jwt(user_id: Uuid, secret: &str) -> String {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: user_id,
        exp: expiration as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .expect("Token generation failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing_and_verification() {
        let password = "mysecretpassword";
        let hashed = hash(password, DEFAULT_COST).expect("Hashing failed");
        
        assert!(verify(password, &hashed).expect("Verification failed"));
        assert!(!verify("wrongpassword", &hashed).expect("Verification failed"));
    }

    #[test]
    fn test_jwt_generation() {
        let user_id = Uuid::new_v4();
        let secret = "test_secret";
        let token = generate_jwt(user_id, secret);
        
        let decoded = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_ref()),
            &Validation::default(),
        ).expect("Failed to decode token");
        
        assert_eq!(decoded.claims.sub, user_id);
    }
}
