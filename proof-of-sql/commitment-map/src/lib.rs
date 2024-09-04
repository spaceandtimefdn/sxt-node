#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod generic_over_commitment;

mod commitment_scheme;
pub use commitment_scheme::{
    AnyCommitmentScheme, CommitmentScheme, CommitmentSchemeFlags, PerCommitmentScheme,
};
