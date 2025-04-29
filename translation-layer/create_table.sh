#!/bin/bash

# API Base URL
BASE_URL="http://127.0.0.1:3000"

# Schema and Table Names
SCHEMA_NAME="TEST_SCHEMA_NAME2"
TABLE_NAME="TEST_TABLE_NAME2"
SOURCE="Sepolia"
MODE="core"

# Step 1: Create Table
echo "🆕 Creating table..."
CREATE_TABLE_RESPONSE=$(curl -s -X POST "$BASE_URL/create_table" \
    -H "Content-Type: application/json" \
    -d "{
        \"schemaName\": \"$SCHEMA_NAME\",
        \"ddlStatement\": \"CREATE TABLE IF NOT EXISTS $SCHEMA_NAME.$TABLE_NAME (BLOCK_NUMBER BIGINT NOT NULL, TIME_STAMP TIMESTAMP);\",
        \"source\": \"$SOURCE\",
        \"mode\": \"$MODE\",
        \"tables\": [
            {
                \"tableName\": \"$TABLE_NAME\",
                \"schemaName\": \"$SCHEMA_NAME\",
                \"ddlStatement\": \"CREATE TABLE IF NOT EXISTS $SCHEMA_NAME.$TABLE_NAME (BLOCK_NUMBER BIGINT NOT NULL, TIME_STAMP TIMESTAMP);\"
            }
        ],
        \"tableName\": \"$TABLE_NAME\"
    }")

# Extract the transaction hash
TX_HASH=$(echo "$CREATE_TABLE_RESPONSE" | jq -r '.txHash')

if [[ -z "$TX_HASH" || "$TX_HASH" == "null" ]]; then
    echo "❌ Failed to create table: $(echo "$CREATE_TABLE_RESPONSE" | jq -r '.errMsg')"
    exit 1
fi

echo "✅ Table creation transaction submitted. Transaction hash: $TX_HASH"

# Step 2: Wait for Transaction Inclusion
echo "🔄 Checking transaction inclusion in the blockchain..."
BLOCK_HASH=""

while [[ -z "$BLOCK_HASH" || "$BLOCK_HASH" == "null" ]]; do
    sleep 1  # Wait 1 second before retrying
    INCLUSION_RESPONSE=$(curl -s -X GET "$BASE_URL/get_extrinsic_status?tx_hash=$TX_HASH")

    BLOCK_HASH=$(echo "$INCLUSION_RESPONSE" | jq -r '.status.finalized_in_block')
    STATUS_DETAILS=$(echo "$INCLUSION_RESPONSE" | jq -r '.status')

    echo "📡 Latest Status: $STATUS_DETAILS"

    if [[ -z "$BLOCK_HASH" || "$BLOCK_HASH" == "null" ]]; then
        echo "⏳ Transaction not finalized yet, checking again..."
    else
        echo "✅ Transaction finalized in block: $BLOCK_HASH"
        break
    fi
done

# Step 3: Verify Execution in Block
echo "🔍 Verifying table creation transaction execution in block..."
EXECUTION_RESPONSE=$(curl -s -X GET "$BASE_URL/get_extrinsic_status_in_block?tx_hash=$TX_HASH&block_hash=$BLOCK_HASH")

# Parse execution status
EXECUTION_SUCCESS=$(echo "$EXECUTION_RESPONSE" | jq -r '.success')
EXECUTION_DETAILS=$(echo "$EXECUTION_RESPONSE" | jq -r '.details')

if [[ "$EXECUTION_SUCCESS" == "true" ]]; then
    echo "✅ Table creation executed successfully in block $BLOCK_HASH."
else
    echo "❌ Table creation failed: $EXECUTION_DETAILS"
    exit 1
fi

echo "🎉 Table successfully created and indexed!"

# Step 4: Drop Table
echo "🗑️ Dropping table..."
DROP_TABLE_RESPONSE=$(curl -s -X POST "$BASE_URL/drop_table" \
    -H "Content-Type: application/json" \
    -d "{
        \"schemaName\": \"$SCHEMA_NAME\",
        \"tableName\": \"$TABLE_NAME\",
        \"source\": \"$SOURCE\",
        \"mode\": \"$MODE\"
    }")

# Extract the drop transaction hash
DROP_TX_HASH=$(echo "$DROP_TABLE_RESPONSE" | jq -r '.txHash')

if [[ -z "$DROP_TX_HASH" || "$DROP_TX_HASH" == "null" ]]; then
    echo "❌ Failed to drop table: $(echo "$DROP_TABLE_RESPONSE" | jq -r '.errMsg')"
    exit 1
fi

echo "✅ Table drop transaction submitted. Transaction hash: $DROP_TX_HASH"

# Step 5: Wait for Drop Transaction Inclusion
echo "🔄 Checking table drop transaction inclusion in the blockchain..."
DROP_BLOCK_HASH=""

while [[ -z "$DROP_BLOCK_HASH" || "$DROP_BLOCK_HASH" == "null" ]]; do
    sleep 1  # Wait 1 second before retrying
    DROP_INCLUSION_RESPONSE=$(curl -s -X GET "$BASE_URL/get_extrinsic_status?tx_hash=$DROP_TX_HASH")

    DROP_BLOCK_HASH=$(echo "$DROP_INCLUSION_RESPONSE" | jq -r '.status.finalized_in_block')
    DROP_STATUS_DETAILS=$(echo "$DROP_INCLUSION_RESPONSE" | jq -r '.status')

    echo "📡 Latest Drop Status: $DROP_STATUS_DETAILS"

    if [[ -z "$DROP_BLOCK_HASH" || "$DROP_BLOCK_HASH" == "null" ]]; then
        echo "⏳ Drop transaction not finalized yet, checking again..."
    else
        echo "✅ Drop transaction finalized in block: $DROP_BLOCK_HASH"
        break
    fi
done

# Step 6: Verify Drop Execution in Block
echo "🔍 Verifying table drop transaction execution in block..."
DROP_EXECUTION_RESPONSE=$(curl -s -X GET "$BASE_URL/get_extrinsic_status_in_block?tx_hash=$DROP_TX_HASH&block_hash=$DROP_BLOCK_HASH")

# Parse execution status
DROP_EXECUTION_SUCCESS=$(echo "$DROP_EXECUTION_RESPONSE" | jq -r '.success')
DROP_EXECUTION_DETAILS=$(echo "$DROP_EXECUTION_RESPONSE" | jq -r '.details')

if [[ "$DROP_EXECUTION_SUCCESS" == "true" ]]; then
    echo "✅ Table drop executed successfully in block $DROP_BLOCK_HASH."
else
    echo "❌ Table drop failed: $DROP_EXECUTION_DETAILS"
    exit 1
fi

echo "🎉 Table successfully created and then dropped!"
