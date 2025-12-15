#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod generic_over_commitment;

/// Function trait for operations generic over commitment schemes.
mod generic_over_commitment_fn;
pub use generic_over_commitment_fn::GenericOverCommitmentFn;

/// Commitment scheme types and configuration.
mod commitment_scheme;
pub use commitment_scheme::{
    AnyCommitmentScheme,
    CommitmentId,
    CommitmentScheme,
    CommitmentSchemeFlags,
    PerCommitmentScheme,
};

/// Implementation details for commitment maps.
mod commitment_map_implementor;

/// Core commitment map trait and implementations.
mod commitment_map;
pub use commitment_map::{CommitmentMap, CommitmentSchemesMismatchError, KeyExistsError};

#[cfg(feature = "memory-commitment-map")]
mod memory_commitment_map;
#[cfg(feature = "memory-commitment-map")]
pub use memory_commitment_map::MemoryCommitmentMap;

#[cfg(feature = "substrate")]
mod commitment_storage_map;
#[cfg(feature = "substrate")]
pub use commitment_storage_map::{
    CommitmentStorageMapHandler,
    TableCommitmentBytes,
    TableCommitmentBytesPerCommitmentScheme,
    TableCommitmentBytesPerCommitmentSchemePassBy,
    TableCommitmentMaxLength,
    TableCommitmentPerCommitmentScheme,
    TableCommitmentToBytesError,
};

#[cfg(feature = "proptest")]
pub mod proptest;
