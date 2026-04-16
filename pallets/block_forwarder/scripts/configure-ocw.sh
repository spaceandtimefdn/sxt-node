#!/usr/bin/env bash
#
# Configure a running sxt-node's offchain worker by writing the indexer URL
# to offchain persistent storage. Run this AFTER the node is up.
#
# Usage:
#   ./configure-ocw.sh [indexer-url] [node-rpc-url]
#
# Defaults:
#   indexer-url:   http://127.0.0.1:9999
#   node-rpc-url:  http://127.0.0.1:9933

set -euo pipefail

INDEXER_URL="${1:-http://127.0.0.1:9999}"
NODE_RPC="${2:-http://127.0.0.1:9944}"

# Hex-encode a string (UTF-8 bytes → "0x...").
hex_of() {
  printf '%s' "$1" | od -An -tx1 | tr -d ' \n' | sed 's/^/0x/'
}

# SCALE-encode a Vec<u8>: compact length prefix + raw bytes.
# For short strings (<64 bytes) the compact prefix is (len << 2), single byte.
scale_encode_vec_u8() {
  local str="$1"
  local len=${#str}
  if (( len >= 64 )); then
    echo "URL too long for simple SCALE encoding (>=64 bytes)" >&2
    exit 1
  fi
  # Compact encoding: (len << 2) as a single byte
  local prefix_byte
  prefix_byte=$(printf '%02x' $((len << 2)))
  local body_hex
  body_hex=$(printf '%s' "$str" | od -An -tx1 | tr -d ' \n')
  printf '0x%s%s' "$prefix_byte" "$body_hex"
}

KEY_HEX=$(hex_of "block_forwarder::indexer_url")
VALUE_HEX=$(scale_encode_vec_u8 "$INDEXER_URL")

echo "Node RPC:     $NODE_RPC"
echo "Indexer URL:  $INDEXER_URL"
echo "Storage key:  $KEY_HEX"
echo "Storage val:  $VALUE_HEX"
echo

if ! response=$(curl -sS --fail-with-body --connect-timeout 5 -X POST "$NODE_RPC" \
  -H "Content-Type: application/json" \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"offchain_localStorageSet\",\"params\":[\"PERSISTENT\",\"$KEY_HEX\",\"$VALUE_HEX\"]}" 2>&1); then
  echo "✗ RPC call failed. Is the node running at $NODE_RPC?"
  echo "  curl output: $response"
  echo "  Start the node with: ./target/release/sxt-node --dev --tmp --rpc-methods=unsafe --rpc-cors=all --offchain-worker=always"
  exit 1
fi

echo "RPC response: $response"

if echo "$response" | grep -q '"result":null'; then
  echo
  echo "✓ Indexer URL configured. The OCW will start forwarding on the next block."
  echo
  echo "Next steps:"
  echo "  1. Create a table via Polkadot.js Apps:"
  echo "     https://polkadot.js.org/apps/?rpc=ws://127.0.0.1:9944"
  echo "     Developer → Sudo → tables.createTables"
  echo "  2. Watch the mock server terminal for CreateTable + Checkpoint calls"
else
  echo
  echo "✗ Configuration failed. Is the node running with --rpc-methods=unsafe?"
  echo "  offchain_localStorageSet is an unsafe RPC and requires that flag."
  exit 1
fi
