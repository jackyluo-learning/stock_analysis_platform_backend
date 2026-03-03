# Stock Analysis Platform Backend

## Project Overview
This project is the backend for a Stock Analysis Platform, implemented in Rust (2024 edition). 

### Status: Sprint 1 Completed ✅
The core infrastructure and basic features (Authentication, Watchlist, Positions) have been implemented and verified.

## Implemented Features
- **User Authentication:** Registration and Login with password hashing (`bcrypt`) and JWT-based authentication.
- **Stock Watchlist:** Search for stocks via **Finnhub API**, manage (add/remove) personal watchlist symbols in PostgreSQL.
- **Position Management:** Comprehensive tracking of stock holdings (positions) for portfolio analysis.
- **Real-time Price Engine:** Background worker utilizing **Alpaca API batch snapshots** for efficient periodic stock price refreshes with local `dashmap` caching.
- **Database Layer:** Robust PostgreSQL integration using `sqlx` with automated migrations and connection pooling.
- **Enterprise-grade Observability:** Structured JSON logging with `tracing`, request/response tracing, and log rotation for production reliability.
- **API Standards:** CORS support, unified error handling with `anyhow`, and a standard health check endpoint.

## Building and Running
As this is a standard Rust project, use the following commands:
- **Build:** `cargo build`
- **Run:** `cargo run`
- **Test:** `cargo test`
- **Lint:** `cargo clippy`
- **Format:** `cargo fmt`

## Project Structure
- `src/main.rs`: The main entry point and Axum router setup.
- `src/auth.rs`: User registration, login, and JWT logic.
- `src/stocks.rs`: Watchlist management and stock search.
- `src/positions.rs`: Stock position tracking.
- `src/db.rs`: Database connection and pooling.
- `src/db_setup.rs`: Pre-run database verification checks.
- `src/crypto.rs`: Password hashing and cryptographic utilities.
- `src/logging.rs`: Tracing and structured logging configuration.
- `Cargo.toml`: Project configuration and dependency management.

## Verification and Testing
All core components have been verified through a combination of unit tests and integration tests:
- **Unit Tests:**
  - `auth`: Password hashing and JWT generation.
  - `crypto`: AES-256-GCM encryption/decryption.
  - `stocks`: Finnhub response parsing and Alpaca snapshot processing (newly added).
- **Integration Tests:**
  - Manual verification of endpoints via `curl` (e.g., `/stocks/search`, `/auth/login`).
  - Database migrations and connection pool health checked on startup.
