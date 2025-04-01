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
