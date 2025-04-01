# `attestation_tree`
We want to be able to verify that arbitrary chain data was agreed upon in a decentralized manner.
Substrate does implement its storage in the form of a merkle tree, which does accomplish this.
However, a couple other requirements compell us to implement something custom:
- we want to verify data across languages, especially solidity
- we sometimes want to perform some processing on the data so that it's more easily consumable in other langauges, especially solidity

This crate defines the "attestation tree" that attestor nodes multi-sign in our network.
It provides abstractions for defining how a given substrate storage prefix should be encoded into the leafs of the tree.
Then, these abstractions can be used for generating the tree, generating storage keys, and proving arbitrary leaves.
