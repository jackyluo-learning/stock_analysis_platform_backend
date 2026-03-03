# Stock Analysis Platform Backend

A production-grade REST API backend for stock portfolio management, built with Rust using modern async web architecture.

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Web Framework** | Axum 0.7 (with `State` extractor) |
| **Async Runtime** | Tokio (full features) |
| **Database** | PostgreSQL + SQLx 0.7 (compile-time checked queries) |
| **Authentication** | JWT (jsonwebtoken) + bcrypt password hashing |
| **Encryption** | AES-256-GCM (aes-gcm) for sensitive data at rest |
| **Market Data** | Alpaca API (real-time quotes) + Finnhub API (symbol search) |
| **Logging** | tracing + tracing-subscriber (console + daily rolling JSON file) |
| **HTTP Client** | reqwest (shared connection pool) |
| **Caching** | DashMap (lock-free concurrent cache) |

## Project Structure

```
src/
├── main.rs          # Entry point, router, graceful shutdown, health check
├── config.rs        # Typed configuration from environment variables
├── error.rs         # Unified AppError enum with IntoResponse
├── auth.rs          # Registration, login, JWT generation, AuthUser extractor
├── stocks.rs        # Finnhub search, watchlist CRUD, Alpaca real-time worker
├── positions.rs     # Portfolio positions with encrypted details
├── crypto.rs        # AES-256-GCM encrypt/decrypt utilities
├── db.rs            # Database module (reserved)
├── db_setup.rs      # Pre-run database existence check & creation
└── logging.rs       # Layered tracing initialization (console + file)

migrations/
└── 20260222_init.sql   # Users, watchlist, positions tables + triggers
```

## API Endpoints

### Health

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/health` | No | Returns DB connectivity and cache status |

**Response:**
```json
{
  "status": "healthy",
  "database": "connected",
  "cache_entries": 5
}
```

### Authentication

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `POST` | `/auth/register` | No | Register a new user |
| `POST` | `/auth/login` | No | Login, returns JWT token |

**Register:**
```json
// POST /auth/register
{
  "username": "john",
  "email": "john@example.com",
  "password": "securepassword"
}
```

**Login:**
```json
// POST /auth/login
{
  "email": "john@example.com",
  "password": "securepassword"
}
// Response:
{ "token": "eyJhbGciOi..." }
```

### Stock Search

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/stocks/search?q=AAPL` | No | Search symbols via Finnhub |

### Watchlist

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/watchlist` | Bearer | Get user's watchlist with live prices |
| `POST` | `/watchlist` | Bearer | Add symbol to watchlist |
| `DELETE` | `/watchlist/:symbol` | Bearer | Remove symbol from watchlist |

**Add to watchlist:**
```json
// POST /watchlist
// Authorization: Bearer <token>
{ "symbol": "AAPL" }
```

**Watchlist response:**
```json
[
  {
    "symbol": "AAPL",
    "price": 150.25,
    "change": 2.50,
    "change_percent": 1.69
  }
]
```

### Positions

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET` | `/positions` | Bearer | Get user's portfolio positions |
| `POST` | `/positions` | Bearer | Add a new position |

**Add position:**
```json
// POST /positions
// Authorization: Bearer <token>
{
  "symbol": "AAPL",
  "shares": "10.5",
  "cost_basis": "145.00",
  "details": {
    "broker": "Interactive Brokers",
    "notes": "Long-term hold"
  }
}
```

> **Note:** The `details` field is encrypted at rest using AES-256-GCM.

## Error Handling

All errors return a consistent JSON structure:

```json
{
  "error": "unauthorized",
  "message": "Invalid or expired token"
}
```

| Error Type | HTTP Status | Meaning |
|-----------|-------------|---------|
| `database_error` | 500 | Database query or connection failure |
| `external_api_error` | 502 | Alpaca/Finnhub API call failure |
| `unauthorized` | 401 | Missing/invalid token or credentials |
| `bad_request` | 400 | Validation error or duplicate resource |
| `crypto_error` | 500 | Encryption/decryption failure |
| `internal_error` | 500 | Generic internal error |

## Setup

### Prerequisites

- Rust (edition 2024)
- PostgreSQL database
- Alpaca API account (for real-time market data)
- Finnhub API account (for symbol search)

### Environment Variables

Create a `.env` file in the project root:

```env
# Required
DATABASE_URL=postgres://user:password@host/stock_platform?sslmode=disable
JWT_SECRET=your_jwt_secret_here
ENCRYPTION_KEY=64_char_hex_string_for_aes256_key

# Market Data APIs
ALPACA_API_KEY=your_alpaca_api_key
ALPACA_API_SECRET=your_alpaca_api_secret
FINNHUB_API_KEY=your_finnhub_api_key

# Optional (defaults shown)
RUST_LOG=info
SERVER_HOST=0.0.0.0
SERVER_PORT=3000
MAX_DB_CONNECTIONS=5
```

### Run

```bash
# Development
cargo run

# Or use the provided start script
chmod +x start.sh
./start.sh
```

The server starts on `http://0.0.0.0:3000` by default.

### Tests

```bash
cargo test
```

```
running 9 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_parse_encryption_key_fallback ... ok
test config::tests::test_parse_encryption_key_hex ... ok
test stocks::tests::test_parse_finnhub_search_response ... ok
test crypto::tests::test_encryption_decryption_roundtrip ... ok
test crypto::tests::test_decryption_with_wrong_key_fails ... ok
test stocks::tests::test_process_alpaca_snapshots ... ok
test auth::tests::test_jwt_generation ... ok
test auth::tests::test_password_hashing_and_verification ... ok

test result: ok. 9 passed; 0 failed
```

## Architecture Highlights

- **State management**: Axum `State` extractor with `Arc<AppState>` for compile-time type safety (not legacy `Extension`)
- **Unified error type**: `AppError` enum implements `IntoResponse` with `From` impls for `?` operator propagation
- **Typed configuration**: `AppConfig` struct loaded from env vars with defaults and validation
- **Shared HTTP client**: Single `reqwest::Client` with connection pooling across all handlers and background worker
- **Graceful shutdown**: Listens for `SIGTERM` / `Ctrl+C` to cleanly terminate connections
- **Background worker**: Tokio-spawned task polls Alpaca snapshots every 10s, batch-fetches all watchlist symbols in one request
- **Structured logging**: Dual-layer tracing — human-readable console + machine-parseable JSON file with daily rotation
- **Encryption at rest**: Position details stored as AES-256-GCM encrypted blobs in PostgreSQL

## Database Schema

```
users
├── id (UUID, PK)
├── username (TEXT, UNIQUE)
├── email (TEXT, UNIQUE)
├── password_hash (TEXT)
├── created_at (TIMESTAMPTZ)
└── updated_at (TIMESTAMPTZ, auto-trigger)

watchlist
├── user_id (UUID, FK → users)
├── symbol (TEXT)
├── market (TEXT, default 'US')
└── created_at (TIMESTAMPTZ)
    PK: (user_id, symbol, market)

positions
├── id (UUID, PK)
├── user_id (UUID, FK → users)
├── symbol (TEXT)
├── shares (DECIMAL 18,4)
├── cost_basis (DECIMAL 18,4)
├── encrypted_details (BYTEA)
├── created_at (TIMESTAMPTZ)
└── updated_at (TIMESTAMPTZ, auto-trigger)
```
