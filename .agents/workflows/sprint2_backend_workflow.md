---
description: Sprint 2 Backend Workflow - Linked Calculation and Market Data
---

# Sprint 2 Backend Execution Workflow

This workflow outlines the systematic execution of Sprint 2 Backend Requirements for the Stock Analysis Platform. 
Sprint 2 focuses on Epic 4: Portfolio Management (Linked Calculation Engine) and expanding market data coverage.

## Prerequisites
- Rust (2024 edition) configured.
- PostgreSQL database running (from Sprint 1 setup).
- Alpaca and Finnhub API keys setup in `.env`.

## Step 1: Database Schema Expansion for Fractional Shares & P&L
Update the database to handle fractional shares with strict 0.0001 precision and establish a foundation for accurate P&L calculation.
- Create a new SQL migration to modify the `positions` table.
- Change tracking columns (e.g., `quantity`, `average_price`) to `NUMERIC(15, 4)` to support exact decimal precision.
- **Crucial Rule**: The `rust_decimal::Decimal` type must be utilized across all entity structs and `sqlx` query mappings to strictly prevent IEEE 754 floating-point inaccuracies.

## Step 2: Implement Linked Calculation Engine (Mode A & B)
Implement the core financial logic to evaluate user positions based on the latest market prices.
- **Goal**: Support fractional trades and calculate real-time Floating Profit/Loss (P&L).
- **Mode A (Event-Driven)**: Setup hooks where position calculations are triggered immediately upon receiving individual real-time price updates.
- **Mode B (Periodic Batch)**: Connect the engine to the existing Alpaca background worker designed in Sprint 1 to perform batch recalcs across all user positions.

## Step 3: Expand Market Data Service Capabilities
Enhance the existing market integrations (Finnhub/Alpaca) to satisfy new detailed display requirements (US.7 & US.4).
- **Basic Stock Info**: Extend the stock detail endpoint to return Open, Close, High, Low, and 52-week range data.
- **Post-Market Data (US.4)**: Update the data fetch mechanisms (like the Alpaca snapshot endpoint) to explicitly include extended-hours (post-market) pricing data when the market is closed.

## Step 4: Develop Personal Holding & P&L APIs
Create performant REST endpoints serving the calculated portfolio data to the frontend (US.10).
- **Endpoint**: Implement `GET /positions/summary` or similar.
- Response should aggregate core metrics: Total Account Value, Daily P&L, Total floating P&L, and a list of detailed positions reflecting fractional ownership.
- Include structured JSON logging via `tracing` to monitor the performance of P&L aggregations.

## Step 5: Robust Verification & Testing
Ensure correctness, zero-cost abstractions, and memory safety aligned with the Global Senior Dev Guidelines.
- Write unit tests targeting the Linked Calculation Engine to assert exact fractional math outcomes under edge conditions.
- Implement integration tests mimicking Alpaca batch snapshot feeds to ensure the overall pipeline (Data -> Cache -> Calculation Engine -> API) executes efficiently.
- Ensure all Rust `Result` types are explicitly handled without using unwraps.
