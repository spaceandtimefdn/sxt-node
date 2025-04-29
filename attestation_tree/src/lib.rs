#![doc = include_str!("../README.md")]

mod hash_and_key;
pub use hash_and_key::{HashAndKey, HashAndKeyTuple};

mod prefix_foliate;
pub use prefix_foliate::{
    decode_storage_key_and_value,
    storage_key_for_prefix_key_tuple,
    DecodeStorageError,
    PrefixFoliate,
};

mod commitment_map_prefix_foliate;
pub use commitment_map_prefix_foliate::CommitmentMapPrefixFoliate;

mod locks_staking_prefix_foliate;
pub use locks_staking_prefix_foliate::{LocksStakingPrefixFoliate, STAKING_BALANCE_LOCK_ID};

mod attestation_tree;
pub use attestation_tree::{
    attestation_tree_from_prefixes,
    prove_leaf_pair,
    AttestationTreeError,
    AttestationTreeProofError,
};
