# Watcher

watcher is a utility program designed for attestors in the SxT network. It facilitates the attestation process by securely signing and submitting attestations using Ethereum and Substrate private keys.
Key Setup

The program requires two keys:

    An Ethereum-style private key for signing attestations.
    A Substrate private key for submitting transactions to the SxT blockchain.

Both keys should be in hex-encoded bytes format. By default, the program looks for:

    Ethereum key: eth.key in the current working directory.
    Substrate key: substrate.key in the current working directory.

## Custom Key Paths

To specify custom paths for the keys, use the following flags:

    --eth-key-path <path>: Specify the Ethereum key location.
    --substrate-key-path <path>: Specify the Substrate key location.

## Generating a Substrate Key

You can generate a Substrate private key using the Subkey CLI tool. Run the following command:

```shell
$ subkey generate
```

Example output:

```shell
Secret phrase:       pledge fix tomato wrist world another under silk peanut risk process disease
  Network ID:        substrate
  Secret seed:       0xab97c9add37a64503844969b5b6f261e47e88bab1acf9b63ab52c56b32192b93
  Public key (hex):  0x8c0b429a499e7e3f38fcaf75d679ac2d3ddccbfc14a56e9815a11b1135bfcf4b
  Account ID:        0x8c0b429a499e7e3f38fcaf75d679ac2d3ddccbfc14a56e9815a11b1135bfcf4b
  Public key (SS58): 5FEKuVze2vPmWXkHpab7NigVFhx6nixEiYSZig1Jfy3ubNZg
  SS58 Address:      5FEKuVze2vPmWXkHpab7NigVFhx6nixEiYSZig1Jfy3ubNZg
```
To save the private key in the required format, you can use this script:

```shell
$ subkey generate | grep "Secret seed" | awk '{print substr($NF, 3)}' > substrate.key
```

This will create a file substrate.key containing only the hex-encoded private key.
Generating an Ethereum Key

You can generate an Ethereum private key using openssl. Run the following command:

```shell
$ openssl rand --hex 32 > eth.key
```

This will generate 32 random bytes in hex format and save them to eth.key.

## Registration

To become an attestor on the SxT network, you must register your account. This involves sending your registration details to an SxT network admin.
Generate Registration Details

Run the following command:

```shell
$ watcher register
```

Example output:
```shell
[INFO] Send these registration details to an SxT network admin
    account_id=5Co821TSjiFVtyXzQraZW7Fwfjhyjd33bwb7yKgLfiBeXr7f
    r=0x88ffc08770071e7e228fad472159a79545c2613cb9b35716998e53fd1f05d9a5
    s=0x15a499ae357ec9eaf49014508ba1b9f0d15766b4a8243bc96ab652c8d37b417a
    v=0x0
    pub_key=0x02ba73c7c2e0b23f64d444a566dfc797744aa113c55e155bc4e6a35657a8fc5a70
```
account_id: Your Substrate account ID.
r, s, v: Ethereum signature components.
pub_key: Your Ethereum public key.

Send these details to an SxT network admin. Once approved and added to the chain's set of attestors, you can start submitting attestations.

## Attesting

Once registered and funded with SxT tokens, you can start attesting finalized blocks.
Start Attesting

Run the following command:

```shell
$ watcher run
```

### Custom WebSocket Address

To specify a custom WebSocket endpoint for the Substrate node, use the --websocket flag:

```shell
$ watcher --websocket ws://your-node-url:9944 run
```
This connects the program to your specified Substrate node.

### Command Reference

    watcher register: Generates registration details for attestors.
    watcher run: Starts attesting finalized blocks in real-time.
    --eth-key-path <path>: Specifies the path to the Ethereum private key file.
    --substrate-key-path <path>: Specifies the path to the Substrate private key file.
    --websocket <url>: Specifies the WebSocket URL of the Substrate node.

### Requirements

    Rust (for building the program)
    Subkey (for generating Substrate keys)
    OpenSSL (for generating Ethereum keys)