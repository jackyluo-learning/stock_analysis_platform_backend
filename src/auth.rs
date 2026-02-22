use axum::{
    extract::{Json, FromRequestParts},
    http::{StatusCode, request::Parts},
    response::IntoResponse,
    Extension, async_trait,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;
use uuid::Uuid;
use chrono::{Utc, Duration};

#[derive(Debug)]
pub struct AuthUser {
    pub id: Uuid,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(state): Extension<Arc<AppState>> = Extension::from_request_parts(parts, _state)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        if !auth_header.starts_with("Bearer ") {
            return Err(StatusCode::UNAUTHORIZED);
        }

        let token = &auth_header[7..];
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.jwt_secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

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
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> impl IntoResponse {
    let password_hash = match hash(payload.password, DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Error hashing password").into_response(),
    };

    let result = sqlx::query(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3)"
    )
    .bind(payload.username)
    .bind(payload.email)
    .bind(password_hash)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => (StatusCode::CREATED, "User registered").into_response(),
        Err(e) => {
            tracing::error!("Registration error: {}", e);
            (StatusCode::BAD_REQUEST, "User already exists or database error").into_response()
        }
    }
}

pub async fn login(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let user = sqlx::query_as::<_, UserRecord>(
        "SELECT id, password_hash FROM users WHERE email = $1"
    )
    .bind(&payload.email)
    .fetch_optional(&state.db)
    .await;

    match user {
        Ok(Some(user)) => {
            if verify(payload.password, &user.password_hash).unwrap_or(false) {
                let token = generate_jwt(user.id, &state.jwt_secret);
                (StatusCode::OK, Json(AuthResponse { token })).into_response()
            } else {
                (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response()
            }
        }
        _ => (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response(),
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
