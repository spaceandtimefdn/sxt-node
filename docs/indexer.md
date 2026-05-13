# **Indexer Setup**

## 1\. Prerequisites

Every SXT Indexer should be separately managed and resourced.

Generation of private/public keys is required, please manage them appropriately. Likewise every SXT Indexer requires its own set of keys.

There are 2 types of indexers:

* Indexer \-\> System Contract Indexer
* SCI Indexer \-\> Multiple Smart Contract Indexer

### 1.1 System Specifications

The minimum system requirements for running a SXT Indexer are shown in the table below:

| Indexer (Staking or Messaging) |                 |
| ------------------------------ | --------------- |
| **Key**                        | **Value**       |
| **CPU cores**                  | 2               |
| **CPU Architecture**           | amd64           |
| **Memory (GiB)**               | 4               |
| **Storage (GiB)**              | N/A             |
| **Storage Type**               | N/A             |
| **OS**                         | Linux           |
| **Network Speed**              | 500Mbps up/down |

| SCI Indexer          |                 |
| -------------------- | --------------- |
| **Key**              | **Value**       |
| **CPU cores**        | 8               |
| **CPU Architecture** | amd64           |
| **Memory (GiB)**     | 32              |
| **Storage (GiB)**    | N/A             |
| **Storage Type**     | N/A             |
| **OS**               | Linux           |
| **Network Speed**    | 500Mbps up/down |

##

## 2\. Downloads

### Docker Image

Assuming Docker Desktop is installed and working on your computer. The SXT Attestor Docker image can be downloaded with docker pull command.

Make sure you have [authenticated Docker with the Container registry](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry#authenticating-to-the-container-registry).

```
docker pull ghcr.io/spaceandtimefdn/sxt-indexer:1.18.0
docker images --digests ghcr.io/spaceandtimefdn/sxt-indexer:1.18.0
```

When each new docker image is released we will also be sharing the full sha256 hash of the image. Please confirm that hash against the image pulled down by docker with an extra docker images argument `--digests` to make sure that you are pulling the right one.

## 3\. Indexer Registration

IMPORTANT: Every indexer submitting data to the network will need its own unique ED25519 private key

```bash
docker run -it --rm \
    -v sxt-indexer-key:/key \
    --entrypoint bash \
    --platform linux/amd64 \
    ghcr.io/spaceandtimefdn/sxt-indexer:1.18.0 \
    -c "subkey generate | awk '
        /Secret seed/ {print substr(\$NF, 3) > \"/key/private.key\"}
        /SS58 Address/ {print \$NF > \"/key/public.key\"}'"
```

Send your public.key file to [readiness@sxt.foundation](mailto:readiness@sxt.foundation). Once your key has been registered with the network you’ll be notified.

```
To: readiness@sxt.foundation
Subject: Attestor Registration - [NOP Name]
```

## **4\. Deployment**

### **IMPORTANT:** DO NOT perform this step until you have received an email from the SXT network administrator confirming that your SXT Indexer has been registered and and funded.

Once you have successfully received confirmation of registration, run the following command.

### 4.1 Configure SXT Indexer

Set up environment variables for substrate key and ETH RPC endpoint for

```
export YOUR_RPC_ENDPOINT=[Your end point]
```

### 4.2 Deploy SXT Indexer

Select the appropriate option below based on the SXT Indexer you agreed to deploy

1. #### Indexer \- Staking Contract Indexer

```bash
docker run -it --rm \
    -e ALL_BLOCKS_STARTING_FROM=22242142 \
    -e BLOCKCHAIN=ethereum \
    -e BLOCK_PARALLELISM=1 \
    -e MODE=single-contract-sci \
    -e SCHEMA=SXT_SYSTEM_STAKING \
    -e SCI_JSON_ABI_FILENAME=/opt/abis/sxt_eth_staking_v1.json \
    -e SCI_TRACKED_CONTRACT_ADDRESS=0x93d176dd54FF38b08f33b4Fc62573ec80F1da185 \
    -e SXT_CHAIN_ADDR=wss://rpc.mainnet.sxt.network \
    -e SXT_CHAIN_DELAY=3 \
    -e RPC_ENDPOINT=${YOUR_RPC_ENDPOINT?} \
    --name garfield-indexer \
    -v sxt-indexer-key:/key \
    ghcr.io/spaceandtimefdn/sxt-indexer:1.18.0 \
    --substrate-key-file-path /key/private.key
```

2. #### Indexer \- Messaging Contract Indexer

```bash
docker run -it --rm \
    -e ALL_BLOCKS_STARTING_FROM=22242142 \
    -e BLOCKCHAIN=ethereum \
    -e BLOCK_PARALLELISM=1 \
    -e MODE=single-contract-sci \
    -e SCHEMA=SXT_SYSTEM_STAKING \
    -e SCI_JSON_ABI_FILENAME=/opt/abis/sxt_eth_messaging_v1.json \
    -e SCI_TRACKED_CONTRACT_ADDRESS=0x621C793a9813f8bd91Ce2ab6Ae579566c1fefc40 \
    -e SXT_CHAIN_ADDR=wss://rpc.mainnet.sxt.network \
    -e SXT_CHAIN_DELAY=3 \
    -e RPC_ENDPOINT=${YOUR_RPC_ENDPOINT?} \
    --name garfield-indexer \
    -v sxt-indexer-key:/key \
    ghcr.io/spaceandtimefdn/sxt-indexer:1.18.0 \
    --substrate-key-file-path /key/private.key
```

## **5\. Connecting your indexer to your validator node**

It is possible to connect your indexer to your own validator node, but the validator will need to run in rpc mode as well. To do so:

* Add the following flags to the command arguments
  * \--unsafe-rpc-external
  * \--rpc-cors all
  * \--rpc-port 9944

Make sure you understand the risks of running the node with the `--unsafe-rpc-external` flag and take appropriate measures like setting up a firewall to block external traffic from reaching the rpc port.

After your validator has restarted and finished syncing, update the indexer’s `SXT_CHAIN_ADDR` environment variable to point to your validator/rpc node.

Here’s an example of a `docker-compose.yaml` file that includes the rpc node and a staking contract indexer:

```
 ---
name: 'sxt-mainnet-node'

services:
  sxt-node:
    platform: linux/amd64
    restart: unless-stopped
    image: ghcr.io/spaceandtimefdn/sxt-node:mainnet-v1.17.0
    ports:
      - '9615:9615' # metrics
      - '9944:9944' # rpc
      - '30333:30333' # p2p
    volumes:
      - sxt-mainnet-data:/data
      - sxt-node-key:/node-key
    pid: host
    environment:
      HYPER_KZG_PUBLIC_SETUP_DIRECTORY: /data

    healthcheck:
      test:
        - CMD-SHELL
        - >-
          curl -s localhost:9944/health |
          grep '"isSyncing":false'
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 10m

    command: >
      --base-path /data
      --prometheus-port 9615
      --prometheus-external
      --pool-limit 10240
      --pool-kbytes 1024000
      --chain /opt/chainspecs/mainnet-spec.json
      --keystore-path /key
      --node-key-file /node-key/subkey.key
      --bootnodes "/dns/validator0.mainnet.sxt.network/tcp/30333/p2p/12D3KooWK4MUYTiz8H6gG98JwN3bT11keivvFLYjtwEv5sqhwkAt"
      --bootnodes "/dns/validator.mainnet-sxt.ethernodes.io/tcp/30333/p2p/12D3KooWMEq9xSwmr8uTthCb6KM86av1T7AYe44kDPxWovEqzq1w"
      --bootnodes "/dns/bootnode1.sxt.blockhunters.services/tcp/30333/p2p/12D3KooWN8ZsZDNVr1ooMWJtsqMNHCVaQ4cvxvB8kELqrZeqct79"
      --bootnodes "/dns/bootnode2.sxt.blockhunters.services/tcp/30333/p2p/12D3KooWSvSQNVHGmK965dKcCDGaxtyeY1XPCMwFUSLC8opguG1T"
      --bootnodes "/ip4/51.210.3.173/tcp/30333/p2p/12D3KooWRUd3BqRyiGfhxVb2BSyUDLK5nHHNXTddZpqzqvQ73C9u"
      --bootnodes "/ip4/141.95.65.179/tcp/30683/p2p/12D3KooWQ8xPXuBww4qSumnjycjjKDThFUj4nDgGS3UPoLyRBvqJ"
      --validator
      --port 30333
      --log info
      --telemetry-url 'wss://telemetry.polkadot.io/submit/ 5'
      --no-private-ipv4
      --unsafe-rpc-external
      --rpc-cors all
      --rpc-port 9944

  indexer:
    platform: linux/amd64
    image: ghcr.io/spaceandtimefdn/sxt-indexer:1.18.0
    container_name: indexer
    depends_on:
      sxt-node:
        condition: service_healthy
    stdin_open: true
    tty: true
    volumes:
      - sxt-indexer-key:/key
    environment:
      ALL_BLOCKS_STARTING_FROM: 22242142
      BLOCKCHAIN: ethereum
      NAMESPACE: SXT_SYSTEM_STAKING
      BLOCK_PARALLELISM: 1
      MODE: single-contract-sci
      SCHEMA: SXT_SYSTEM_STAKING
      SCI_CONTRACT_NAME: STAKING
      SCI_JSON_ABI_FILENAME: /opt/abis/sxt_eth_staking_v1.json
      SCI_TRACKED_CONTRACT_ADDRESS: 0x93d176dd54FF38b08f33b4Fc62573ec80F1da185
      SXT_CHAIN_ADDR: ws://sxt-node:9944
      SXT_CHAIN_DELAY: 3
      RPC_ENDPOINT: ${YOUR_ETHEREUM_RPC_ENDPOINT}
    command: >
      --substrate-key-file-path /key/private.key

volumes:
  sxt-mainnet-data:
    external: true
  sxt-node-key:
    external: true
  sxt-indexer-key:
    external: true
```
