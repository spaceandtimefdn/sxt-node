#!/usr/bin/bash
# This script, assumes you are running from the root of the sxt-node project directory. It downloads the mainnet 
# snapshot into a local docker container, runs the container, then executes the fork binary against the running 
# container.

# Where to find utility scripts that we'll use such as wait-for-node.sh
UTILS_DIR=utils

# Download the snapshot into the docker container
docker run -it --rm \
  --platform linux/amd64 \
  -v sxt-mainnet-data:/data \
  --entrypoint=bash \
  ghcr.io/spaceandtimefdn/sxt-node:mainnet-v1.17.0 \
  -c '
    set -ex
    # Optional step if you want to keep a backup of the old database, can be deleted later
    if [ -d /data/chains ]; then
      mv /data/chains $(mktemp -d -p /data backup.XXXX)
    fi

    # Download and unpack the snapshot
    if [ ! -d /data/chains ]; then
      cd /data
      curl -O "https://blocks.testnet.sxt.network/snapshots/2026-01-29/sxt-mainnet.tar.gz"
      curl -O "https://blocks.testnet.sxt.network/snapshots/2026-01-29/sxt-mainnet.sha1"
      sha1sum --check sxt-mainnet.sha1
      mkdir -p chains
      tar xvf sxt-mainnet.tar.gz -C chains
      rm sxt-mainnet.tar.gz sxt-mainnet.sha1
      cd -
    fi
  ';

# Start a node from dockerfile with the mainnet chainspec, no peers, so that it does not progress.
# This is important for exporting the state without accidentally pruning it mid-export.
docker run -d --restart always \
  --platform linux/amd64 \
  -v sxt-mainnet-data:/data \
  -v sxt-validator-key:/key \
  -v sxt-node-key:/node-key \
  -p 30333:30333/tcp \
  -p 9615:9615/tcp \
  -p 9944:9944/tcp \
  --env HYPER_KZG_PUBLIC_SETUP_DIRECTORY=/data \
  ghcr.io/spaceandtimefdn/sxt-node:mainnet-v1.17.0 \
  --base-path /data \
  --chain /opt/chainspecs/mainnet-spec.json \
  --keystore-path /key \
  --hyper-kzg-public-setup-release-degree "02" \
  --hyper-kzg-public-setup-sha256 1821173e2452afb5ad77ff8ef740140cd5e57b9b847d8b6edb81e04897b1efe4 \
  --node-key-file /node-key/subkey.key;

# Run 'wait-for-node.sh'
bash ${UTILS_DIR}/wait-for-node.sh;

# download and run creditcoin-fork
if [ ! -f ./creditcoin-fork ]; then
  echo "Downloading fork utility!";
  curl https://github.com/gluwa/creditcoin-fork/releases/download/0.2.1/creditcoin-fork -o creditcoin-fork;
  chmod +x creditcoin-fork;
fi

# Create the fork by exporting the state and writing it to a new chainspec file
./creditcoin-fork --bin target/release/sxt-node --orig chainspecs/raw/mainnet-spec.json -o fork.json;

# Start a node with the newly forked chainspec as alice, so we produce blocks, and using the node binary 
# that generated the chain spec
./target/release/sxt-node --chain fork.json --alice --unsafe-force-node-key-generation;
