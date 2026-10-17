#!/usr/bin/bash
# This is an example command to fork a node running on localhost that is synced to SxT Mainnet.
# The fork will be based on dev, but maintain the state from SxT Mainnet allowing for tests of 
# runtime upgrades and storage migrations
./creditcoin-fork --bin target/release/sxt-node --orig chainspecs/raw/mainnet-spec.json
