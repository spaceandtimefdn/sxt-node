#![doc = include_str!("../README.md")]
#![warn(unused_crate_dependencies)]

/// Hash and key types for attestation tree nodes.
mod hash_and_key;
pub use hash_and_key::{HashAndKey, HashAndKeyTuple};

/// Prefix foliate implementation for storage key iteration.
mod prefix_foliate;
pub use prefix_foliate::{
    decode_storage_key_and_value,
    storage_key_for_prefix_key_tuple,
    DecodeStorageError,
    PrefixFoliate,
};

/// Commitment map prefix foliate implementation.
mod commitment_map_prefix_foliate;
pub use commitment_map_prefix_foliate::CommitmentMapPrefixFoliate;

/// Core attestation tree construction and proof generation.
mod attestation_tree;
pub use attestation_tree::{
    attestation_tree_from_prefixes,
    prove_leaf_pair,
    AttestationTreeError,
    AttestationTreeProofError,
};
