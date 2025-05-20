# SXT Chain Validator Setup Instructions

## Introduction
### Launching SXT Chain – Background
#### Introduction to SXT Chain
* **Overview**: Space and Time (SXT) scales zero-knowledge (ZK) proofs on a decentralized data warehouse to deliver trustless data processing to smart contracts. You can use SXT to join comprehensive blockchain data we've indexed from major chains with apps’ data or other offchain datasets. Proof of SQL is SXT's sub-second ZK coprocessor, which allows smart contracts to ask complicated questions about activity on its own chain or other chains and get back a ZK-proven answer in the next block. SXT enables a new generation of smart contracts that can transact in real time based on both onchain data (like txns, blocks, smart contract events, storage slot changes, etc.) and data from apps, ultimately delivering a more robust onchain economy and more sophisticated onchain applications.
* **Validator activities**: The SXT Chain is designed to witness and validate indexed data submitted by indexer nodes. While the current mainnet phase only involves third-party validators (and not prover nodes quite yet), the SXT Chain ensures the integrity of data through cryptographic commitments—one special hash for each indexed table. As rows of data are inserted on the SXT Chain, the validators update these special hashes (commitments) and add them to each block. Ultimately, this proves/ensures that each table managed by the chain is cryptographically tamperproof.
* **Data Integrity**: As the SXT Chain indexes data, it generates cryptographic hashes (commitments) that guarantee the underlying data tables remain untampered. These commitments are essential for later phases, where our SXT Chain prover nodes will employ our Proof of SQL protocol to validate the integrity of the indexed data during ZK-proven SQL execution for client queries.

## Detailed steps for Validator onboarding on each network
* [mainnet](docs/mainnet.md)
* [testnet](docs/testnet.md)
