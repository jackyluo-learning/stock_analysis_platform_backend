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

echo "--- Verification: Testing Health Endpoint ---"
HEALTH_RESP=$(curl -s http://localhost:3000/health)

if [ "$HEALTH_RESP" == "OK" ]; then
    echo "SUCCESS: Application is healthy (HTTP 200, Response: OK)."
else
    echo "FAILURE: Health check failed. Response: $HEALTH_RESP"
fi

echo "--- Verification Complete ---"
echo "Application is running (PID: $APP_PID). Use 'kill $APP_PID' to stop it."

# Keep the script running to tail the logs
wait $APP_PID
