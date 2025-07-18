# **Attestor Setup**

**Watcher** is a utility program designed for attestors in the **SXT network**. It facilitates the attestation process by securely signing and submitting attestations using Ethereum and Substrate private keys.

## 1\. Prerequisites

Every SXT Attestor should be separately managed and resourced.

Generation of private/public keys is required, please manage them appropriately. Likewise every SXT Attestor requires its own set of keys.

### 1.1 System Specifications

The minimum system requirements for running a SXT Attestor are shown in the table below:

| Attestor             |                 |
| -------------------- | --------------- |
| **Key**              | **Value**       |
| **CPU cores**        | 2               |
| **CPU Architecture** | amd64           |
| **Memory (GiB)**     | 2               |
| **Storage (GiB)**    | N/A             |
| **Storage Type**     | N/A             |
| **OS**               | Linux           |
| **Network Speed**    | 500Mbps up/down |

## 2\. Downloads

### Docker Image

Assuming Docker Desktop is installed and working on your computer. The SXT Attestor Docker image can be downloaded with docker pull command.

Make sure you have [authenticated Docker with the Container registry](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry#authenticating-to-the-container-registry).

```
docker pull ghcr.io/spaceandtimefdn/sxt-attestor:mainnet-v1.7.0
docker images --digests ghcr.io/spaceandtimefdn/sxt-attestor:mainnet-v1.7.0
```

When each new docker image is released we will also be sharing the full sha256 hash of the image. Please confirm that hash against the image pulled down by docker with an extra docker images argument `--digests` to make sure that you are pulling the right one.

## 3\. Attestor Registration

The program requires two keys:

* An **Ethereum-style private key** for signing attestations.

* A **Substrate private key** for submitting transactions to the SXT blockchain.

Both keys should be in **hex-encoded bytes** format. By default, the program looks for:

* `eth.key` in the current working directory (Ethereum key)
* `substrate.key` in the current working directory (Substrate key)

### 3.1 Generating a Substrate Key

```bash
docker run -it --rm \
    -v sxt-attestor-key:/key \
    --entrypoint bash \
    --platform linux/amd64 \
    ghcr.io/spaceandtimefdn/sxt-attestor:mainnet-v1.7.0 \
    -c "subkey generate | awk '/Secret seed/ {
        print substr(\$NF, 3)
    }' > /key/substrate.key"
```

### 3.2 Generating an Ethereum Key

Generate an Ethereum-style private key using **OpenSSL**:

```bash
docker run -it --rm \
    -v sxt-attestor-key:/key \
    --entrypoint bash \
    --platform linux/amd64 \
    ghcr.io/spaceandtimefdn/sxt-attestor:mainnet-v1.7.0 \
    -c 'openssl rand --hex 32 > /key/eth.key'
```

### 3.3 Registrate Attestor

3.3.1 run the following command:

```bash
docker run -it --rm \
    -v sxt-attestor-key:/key \
    -e RUST_LOG=info \
    ghcr.io/spaceandtimefdn/sxt-attestor:mainnet-v1.7.0 \
    --eth-key-path /key/eth.key \
    --substrate-key-path /key/substrate.key \
    register
```

Output Example:

```
[INFO] Send these registration details to an SxT network adminaccount_id=5DAAnrj7VHTz5uFQzDgX3KpQW8jxJmG6rRZDvY5n8d2czxkHr=0xabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd	s=0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcd	v=0x1pub_key=0x03ffacbde123456789abcdef123456789abcdef123456789abcdef123456789abc
```

* **`account_id`: Your Substrate account ID**
* **`r`, `s`, `v`: Ethereum signature components**
* **`pub_key`: Your Ethereum public key**

3.3.2. Copy this output and send it via email to the SXT Foundation:

```
To: readiness@sxt.foundation
Subject: Attestor Registration - [NOP Name]
```

## **4\. Deployment**

**IMPORTANT:** DO NOT perform this step until you have received an email from the SXT network administrator confirming that your SXT Attestor has been registered and and funded.

Once you have successfully received confirmation of registration, run the following command.

```bash
docker run -it --rm \
    -v sxt-attestor-key:/key \
    -e RUST_LOG=info \
    ghcr.io/spaceandtimefdn/sxt-attestor:mainnet-v1.7.0 \
    --eth-key-path /key/eth.key \
    --substrate-key-path /key/substrate.key \
    --websocket wss://rpc.mainnet.sxt.network \
    run
```
