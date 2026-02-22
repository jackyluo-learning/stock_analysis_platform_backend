# Stock Analysis Platform - API Documentation (Sprint 1)

This documentation provides details for the backend API endpoints implemented in Sprint 1.

## Base URL
`http://localhost:3000`

## Authentication
Most endpoints require a JSON Web Token (JWT) for authentication.
- **Header:** `Authorization: Bearer <your_jwt_token>`

---

## 1. Authentication Endpoints

### Register User
`POST /auth/register`

**Request Body:**
```json
{
  "username": "johndoe",
  "email": "john@example.com",
  "password": "securepassword123"
}
```

**Response:**
- `201 Created`: "User registered"
- `400 Bad Request`: "User already exists or database error"

---

### Login User
`POST /auth/login`

**Request Body:**
```json
{
  "email": "john@example.com",
  "password": "securepassword123"
}
```

**Response (200 OK):**
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

---

## 2. Stock Endpoints

### Search Stocks
`GET /stocks/search?q=<query>`

**Query Parameters:**
- `q`: Search term (e.g., "AAPL", "Apple")

**Response (200 OK):**
Returns a JSON object from Yahoo Finance containing matching tickers and metadata.

---

### Get Watchlist
`GET /watchlist` (Requires Auth)

**Response (200 OK):**
```json
[
  {
    "symbol": "AAPL",
    "price": 185.92,
    "change": 0.0,
    "change_percent": 0.0
  },
  {
    "symbol": "TSLA",
    "price": 193.57,
    "change": 0.0,
    "change_percent": 0.0
  }
]
```
*Note: Prices are refreshed in the background every 2 seconds.*

---

### Add to Watchlist
`POST /watchlist` (Requires Auth)

**Request Body:**
```json
{
  "symbol": "AAPL"
}
```

**Response:**
- `201 Created`
- `500 Internal Server Error`

---

### Remove from Watchlist
`DELETE /watchlist/:symbol` (Requires Auth)

**Path Parameters:**
- `symbol`: The ticker symbol to remove (e.g., "AAPL")

**Response:**
- `204 No Content`
- `500 Internal Server Error`

---

## 3. Position Endpoints (Encrypted Storage)

### Get Positions
`GET /positions` (Requires Auth)

**Response (200 OK):**
```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "symbol": "AAPL",
    "shares": "10.5000",
    "cost_basis": "150.25",
    "details": {
      "broker": "Charles Schwab",
      "notes": "Long term hold"
    }
  }
]
```
*Note: Sensitive details are decrypted on-the-fly from secure database storage.*

---

### Add Position
`POST /positions` (Requires Auth)

**Request Body:**
```json
{
  "symbol": "AAPL",
  "shares": "10.5000",
  "cost_basis": "150.25",
  "details": {
    "broker": "Charles Schwab",
    "notes": "Long term hold"
  }
}
```

**Response:**
- `201 Created`
- `500 Internal Server Error`

---

## Technical Notes for Frontend
- **Real-time Updates:** For Sprint 1, use polling (e.g., every 3 seconds) to `GET /watchlist` to display live prices.
- **Precision:** `shares` and `cost_basis` are returned as strings to maintain decimal precision. Use a library like `big.js` or `decimal.js` in React if performing calculations.
- **CORS:** Enabled for all origins (`*`) for development purposes.
