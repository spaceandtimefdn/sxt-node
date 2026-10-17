#!/bin/bash

# Wait for a Substrate node to be ready
# Note: both WS and HTTP are served via the same port
TARGET_URL=${1:-http://127.0.0.1:9944}
CURL_PARAMS="-H 'Content-Type: application/json' -d '{\"id\":\"1\", \"jsonrpc\":\"2.0\", \"method\": \"state_getMetadata\", \"params\":[]}' $TARGET_URL"

COUNTER=0
MAX_ATTEMPTS=50

# Make sure there is a node running at TARGET_URL
while [[ "$(eval curl -s -o /dev/null -w '%{http_code}' "$CURL_PARAMS")" != "200" && $COUNTER -lt $MAX_ATTEMPTS ]]; do
    echo "INFO: $COUNTER - Node not ready yet..."
    (( COUNTER=COUNTER+1 ))
    sleep 50
done

if [ $COUNTER -ge $MAX_ATTEMPTS ]; then
    echo "ERROR: Node did not become ready after $MAX_ATTEMPTS attempts"
    exit 1
fi

# Verify we can actually get metadata
set -e
eval curl "$CURL_PARAMS" > /dev/null
