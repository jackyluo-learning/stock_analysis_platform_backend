# Stock Analysis Platform Backend

## Project Overview
This project is the backend for a Stock Analysis Platform, implemented in Rust (2024 edition). It provides a robust, scalable infrastructure for managing stock watchlists, tracking personal positions with encrypted details, and real-time price monitoring.

### Status: Sprint 1 Completed ✅ | Sprint 2 in Progress 🏗️
- **Sprint 1:** Core infrastructure, authentication, watchlist, and basic position management are fully implemented and verified.
- **Sprint 2:** Focus is on the Linked Calculation Engine (P&L), fractional share precision using `rust_decimal`, and expanded market data capabilities (OHLC, 52-week range).

## Implemented Features
- **User Authentication:** Secure registration and login using `bcrypt` for password hashing and JWT for stateless session management.
- **Stock Watchlist:** Multi-market support (defaulting to 'US'). Search capabilities via **Finnhub API** and personal watchlist management in PostgreSQL.
- **Position Management:** Comprehensive tracking of stock holdings with support for fractional shares (4-decimal precision).
- **Secure Encrypted Storage:** Sensitive position details (e.g., broker, notes) are stored using **AES-256-GCM (AEAD)** encryption, ensuring user privacy even at the database level.
- **Real-time Price Engine:** Background worker utilizing **Alpaca API batch snapshots** for efficient periodic price refreshes, cached in a thread-safe `DashMap`.
- **Unified Error Handling:** Centralized `AppError` enum with `axum::response::IntoResponse` implementation for consistent, typed API error responses.
- **Enterprise-grade Observability:** Structured JSON logging with `tracing`, daily log rotation via `tracing-appender`, and detailed request/response tracing.
- **Environment-based Configuration:** Typed configuration management using `dotenvy` and structured `AppConfig` for seamless environment transitions.
- **Database Layer:** Robust PostgreSQL integration using `sqlx` with automated migrations, connection pooling, and pre-run schema verification.

## Building and Running
As this is a standard Rust project, use the following commands:
- **Build:** `cargo build`
- **Run:** `cargo run` (Ensure `.env` is configured with valid API keys and database URL)
- **Test:** `cargo test`
- **Lint:** `cargo clippy`
- **Format:** `cargo fmt`

## Project Structure
- `src/main.rs`: Entry point, Axum router setup, and shared `AppState` definition.
- `src/auth.rs`: User identity management, password security, and JWT logic.
- `src/stocks.rs`: Watchlist operations, Finnhub search integration, and the Alpaca real-time worker.
- `src/positions.rs`: Portfolio position tracking with support for encrypted metadata.
- `src/config.rs`: Centralized, typed configuration loading from environment variables.
- `src/error.rs`: Unified application error types and response mappings.
- `src/crypto.rs`: AES-256-GCM encryption/decryption and cryptographic utilities.
- `src/db.rs`: Database connection pooling and health utilities.
- `src/db_setup.rs`: Automated database existence checks and initialization logic.
- `src/logging.rs`: Multi-layer tracing configuration (Console + Rolling JSON files).
- `Cargo.toml`: Project manifest and dependency management (utilizing `sqlx`, `axum`, `tokio`, `rust_decimal`).

## Verification and Testing
The platform is verified through a multi-tier testing strategy:
- **Unit Tests:**
  - `auth`: Password hashing integrity and JWT token lifecycle.
  - `crypto`: AES-256-GCM encryption/decryption round-trip validation.
  - `stocks`: Alpaca snapshot processing and Finnhub response parsing logic.
  - `config`: Environment variable mapping and encryption key parsing.
- **Integration Tests:**
  - Migration integrity verified on startup.
  - Database connection pool health checked via the `/health` endpoint.
  - Manual verification of API endpoints (Auth, Stocks, Watchlist, Positions) using Postman/curl.
