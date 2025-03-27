# Messages Module
One of the implementations in this pallet is for parsing messages we receive via an EVM transaction.

The only currently implemented message is for registering session keys on a given validator. This message takes only
SCALE encoded session keys as the payload.

To retrieve the encoded session keys, call the `author_rotateKeys` endpoint on your validator. This RPC can only be 
called via a localhost connection. Once called, the SCALE encoded session keys are provided as a hexadecimal string.

This string can be placed directly in the transaction to the contract.

Testnet Staking Contract:
https://sepolia.etherscan.io/address/0xdb3be8e4b966d189de54b8cf2e01ef387983dec3#writeContract

Testnet Token Contract:
https://sepolia.etherscan.io/address/0x8A6BBaCBe0b3b9Ea00f80022318c0ad2E07a1fE4