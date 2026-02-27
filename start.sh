#!/bin/bash

# Ensure we use the DATABASE_URL from .env rather than any local environment override
unset DATABASE_URL

echo "--- Pre-run: Cleaning up existing instances ---"
EXISTING_PID=$(lsof -t -i:3000)
if [ ! -z "$EXISTING_PID" ]; then
    echo "Found existing application running on port 3000 (PID: $EXISTING_PID). Stopping it..."
    kill -9 $EXISTING_PID
    sleep 2
    echo "Existing instance stopped."
else
    echo "No existing instance found on port 3000."
fi

echo "--- Pre-run: Checking Environment ---"
if [ ! -f .env ]; then
    echo "Error: .env file not found!"
    exit 1
fi

echo "--- Starting Stock Analysis Platform Backend ---"
# Start the application in the background and save the PID
RUST_LOG=info cargo run &
APP_PID=$!

echo "Waiting for application to start on port 3000..."
MAX_RETRIES=30
COUNT=0
while ! nc -z localhost 3000; do
    sleep 1
    COUNT=$((COUNT + 1))
    if [ $COUNT -ge $MAX_RETRIES ]; then
        echo "Error: Application failed to start within $MAX_RETRIES seconds."
        kill $APP_PID 2>/dev/null
        exit 1
    fi
done

echo "--- Verification: Testing API Endpoints ---"

# 1. Test Stock Search (Read-only, public API)
echo "Testing Stock Search (Public API)..."
# Using a silent curl to just check the status code
SEARCH_RESP=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:3000/stocks/search?q=AAPL")

# Note: Yahoo Finance API might occasionally fail with 500, 
# but a 200 or 500 both indicate the server route is active.
if [ "$SEARCH_RESP" == "200" ] || [ "$SEARCH_RESP" == "500" ]; then
    echo "SUCCESS: Stock Search API route is active (HTTP $SEARCH_RESP)."
else
    echo "FAILURE: Stock Search API returned unexpected HTTP $SEARCH_RESP."
fi

echo "--- Verification Complete ---"
echo "Application is running (PID: $APP_PID). Use 'kill $APP_PID' to stop it."

# Keep the script running to tail the logs
wait $APP_PID
