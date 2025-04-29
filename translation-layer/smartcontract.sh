#!/bin/bash

# API Base URL
BASE_URL="http://127.0.0.1:3000"

# Step 1: Add Smart Contract
echo "🆕 Adding smart contract..."
ADD_CONTRACT_RESPONSE=$(curl -s -X POST "$BASE_URL/add_smartcontract" \
    -H "Content-Type: application/json" \
    -d '{
        "type": "normal",
        "source": "Ethereum",
        "address": "0x1234567890abcdef1234567890abcdef12345678",
        "abi": null,
        "startingBlock": 123456,
        "targetSchema": "CREATE TABLE event_data (...);",
        "contractName": "TestContract",
        "events": [
            {
                "name": "Transfer",
                "signature": "Transfer(indexed address from, indexed address to, uint256 value)",
                "table": "event_transfer"
            },
            {
                "name": "Approval",
                "signature": "Approval(indexed address owner, indexed address spender, uint256 value)",
                "table": "event_approval"
            }
        ]
    }')

# Extract the transaction hash
TX_HASH=$(echo "$ADD_CONTRACT_RESPONSE" | jq -r '.txHash')

if [[ -z "$TX_HASH" || "$TX_HASH" == "null" ]]; then
    echo "❌ Failed to add smart contract: $(echo "$ADD_CONTRACT_RESPONSE" | jq -r '.errMsg')"
    exit 1
fi

echo "✅ Contract added successfully. Transaction hash: $TX_HASH"

# Step 2: Wait for Transaction Inclusion
echo "🔄 Checking transaction inclusion in the blockchain..."
BLOCK_HASH=""

while [[ -z "$BLOCK_HASH" || "$BLOCK_HASH" == "null" ]]; do
    sleep 1  # Wait 5 seconds before retrying
    INCLUSION_RESPONSE=$(curl -s -X GET "$BASE_URL/get_extrinsic_status?tx_hash=$TX_HASH")

    BLOCK_HASH=$(echo "$INCLUSION_RESPONSE" | jq -r '.status.finalized_in_block')
    STATUS_DETAILS=$(echo "$INCLUSION_RESPONSE" | jq -r '.status')

    echo $STATUS_DETAILS | jq

    if [[ -z "$BLOCK_HASH" || "$BLOCK_HASH" == "null" ]]; then
        echo "⏳ Transaction not finalized yet, checking again..."
    else
        echo "✅ Transaction finalized in block: $BLOCK_HASH"
        break
    fi
done

# Step 3: Verify Execution in Block
echo "🔍 Verifying transaction execution in block..."
EXECUTION_RESPONSE=$(curl -s -X GET "$BASE_URL/get_extrinsic_status_in_block?tx_hash=$TX_HASH&block_hash=$BLOCK_HASH")

# Parse execution status
EXECUTION_SUCCESS=$(echo "$EXECUTION_RESPONSE" | jq -r '.success')
EXECUTION_DETAILS=$(echo "$EXECUTION_RESPONSE" | jq -r '.details')

if [[ "$EXECUTION_SUCCESS" == "true" ]]; then
    echo "✅ Transaction executed successfully in block $BLOCK_HASH."
else
    echo "❌ Transaction execution failed: $EXECUTION_DETAILS"
    exit 1
fi

# Step 4: Remove Smart Contract
echo "🗑️ Removing smart contract..."
REMOVE_CONTRACT_RESPONSE=$(curl -s -X POST "$BASE_URL/remove_smartcontract" \
    -H "Content-Type: application/json" \
    -d "{
        \"source\": \"Ethereum\",
        \"address\": \"0x1234567890abcdef1234567890abcdef12345678\"
    }")

# Extract the transaction hash for removal
REMOVE_TX_HASH=$(echo "$REMOVE_CONTRACT_RESPONSE" | jq -r '.txHash')

if [[ -z "$REMOVE_TX_HASH" || "$REMOVE_TX_HASH" == "null" ]]; then
    echo "❌ Failed to remove smart contract: $(echo "$REMOVE_CONTRACT_RESPONSE" | jq -r '.errMsg')"
    exit 1
fi

echo "✅ Smart contract removal initiated. Transaction hash: $REMOVE_TX_HASH"

# Step 5: Wait for Removal Transaction Inclusion
echo "🔄 Checking removal transaction inclusion in the blockchain..."
REMOVE_BLOCK_HASH=""

while [[ -z "$REMOVE_BLOCK_HASH" || "$REMOVE_BLOCK_HASH" == "null" ]]; do
    sleep 1  # Wait 5 seconds before retrying
    REMOVE_INCLUSION_RESPONSE=$(curl -s -X GET "$BASE_URL/get_extrinsic_status?tx_hash=$REMOVE_TX_HASH")

    REMOVE_BLOCK_HASH=$(echo "$REMOVE_INCLUSION_RESPONSE" | jq -r '.status.finalized_in_block')
    REMOVE_STATUS_DETAILS=$(echo "$REMOVE_INCLUSION_RESPONSE" | jq -r '.status')

    echo "📡 Latest Removal Status: $REMOVE_STATUS_DETAILS"

    if [[ -z "$REMOVE_BLOCK_HASH" || "$REMOVE_BLOCK_HASH" == "null" ]]; then
        echo "⏳ Removal transaction not finalized yet, checking again..."
    else
        echo "✅ Removal transaction finalized in block: $REMOVE_BLOCK_HASH"
        break
    fi
done

# Step 6: Verify Removal Execution in Block
echo "🔍 Verifying removal transaction execution in block..."
REMOVE_EXECUTION_RESPONSE=$(curl -s -X GET "$BASE_URL/get_extrinsic_status_in_block?tx_hash=$REMOVE_TX_HASH&block_hash=$REMOVE_BLOCK_HASH")

# Parse execution status
REMOVE_EXECUTION_SUCCESS=$(echo "$REMOVE_EXECUTION_RESPONSE" | jq -r '.success')
REMOVE_EXECUTION_DETAILS=$(echo "$REMOVE_EXECUTION_RESPONSE" | jq -r '.details')

if [[ "$REMOVE_EXECUTION_SUCCESS" == "true" ]]; then
    echo "✅ Smart contract removal executed successfully in block $REMOVE_BLOCK_HASH."
else
    echo "❌ Smart contract removal failed: $REMOVE_EXECUTION_DETAILS"
    exit 1
fi

echo "🎉 Smart contract added and successfully removed!"
