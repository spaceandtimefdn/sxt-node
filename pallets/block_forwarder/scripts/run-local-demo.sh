#!/usr/bin/env bash
#
# Run the mock indexer server and sxt-node side by side for local testing.
# Opens each in a tmux pane so you can see both outputs.
#
# Requires: tmux, cargo, wasm32-unknown-unknown target
#
# Usage: ./run-local-demo.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

if ! command -v tmux >/dev/null 2>&1; then
  echo "tmux not installed. Install with: brew install tmux"
  exit 1
fi

SESSION="sxt-ocw-demo"

if tmux has-session -t "$SESSION" 2>/dev/null; then
  echo "Session '$SESSION' already exists. Attaching..."
  tmux attach -t "$SESSION"
  exit 0
fi

cd "$REPO_ROOT"

tmux new-session -d -s "$SESSION" -n mock-server \
  "echo '=== Mock Indexer Server (port 9999) ==='; cargo run -p mock-indexer-server"

tmux new-window -t "$SESSION" -n node \
  "echo '=== SxT Node (dev mode) ==='; \
   echo 'Waiting 10s for mock server to build...'; sleep 10; \
   ./target/release/sxt-node --dev --tmp --rpc-methods=unsafe --rpc-cors=all --offchain-worker=always"

tmux new-window -t "$SESSION" -n configure \
  "echo '=== Configure OCW ==='; \
   echo 'Waiting 60s for node to start...'; sleep 60; \
   $SCRIPT_DIR/configure-ocw.sh; \
   echo; echo 'Now create a table via polkadot.js Apps.'; \
   exec bash"

echo "Started tmux session '$SESSION' with 3 windows:"
echo "  1. mock-server  — the HTTP indexer mock"
echo "  2. node         — sxt-node in dev mode"
echo "  3. configure    — sets the indexer URL via RPC after the node starts"
echo
echo "Attach with: tmux attach -t $SESSION"
echo "Switch windows: Ctrl-b + window number (0, 1, 2)"
echo "Kill session:   tmux kill-session -t $SESSION"

tmux attach -t "$SESSION"
