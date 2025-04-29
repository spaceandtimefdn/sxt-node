use core::fmt::Debug;

use snafu::Snafu;

use crate::generic_over_commitment::{GenericOverCommitment, OptionType};
use crate::{CommitmentScheme, CommitmentSchemeFlags, PerCommitmentScheme};

/// Cannot create key that already exists.
#[derive(Debug, Snafu, PartialEq, Eq)]
#[snafu(display("cannot create key that already exists: {key:?}"))]
pub struct KeyExistsError<K: Debug> {
    /// Table ref that already exists.
    pub key: K,
}

/// A key cannot be updated with mismatched schemes.
#[derive(Debug, Snafu)]
#[snafu(display(
    "key with schemes {original_schemes:?} cannot be updated with schemes {new_schemes:?}"
))]
pub struct CommitmentSchemesMismatchError {
    /// Schemes defined for the key.
    pub original_schemes: CommitmentSchemeFlags,
    /// Schemes in update.
    pub new_schemes: CommitmentSchemeFlags,
}

/// Abstraction for mappings of keys and commitment schemes to values of type `V`.
pub trait CommitmentMap<K: Debug, V: GenericOverCommitment> {
    /// Returns `true` if this map contains this key and scheme.
    fn has_key_and_scheme(&self, key: &K, scheme: &CommitmentScheme) -> bool;

    /// Returns the commitment schemes this map contains for this key.
    fn schemes_for_key(&self, key: &K) -> CommitmentSchemeFlags;

    /// Returns `true` if this map contains this key.
    fn has_key(&self, key: &K) -> bool;

    /// Returns the commitments in this map for a particular key.
    fn get_commitments(&self, key: &K) -> PerCommitmentScheme<OptionType<V>>;

    /// Update the commitments in this map for a particular key and a combination of schemes.
    ///
    /// Fails if the new commitments do not match the existing commitment schemes for the key.
    fn update_commitments(
        &mut self,
        key: K,
        commitments: PerCommitmentScheme<OptionType<V>>,
    ) -> Result<(), CommitmentSchemesMismatchError>;

    /// Create empty commitment for a particular key and a combination of schemes.
    ///
    /// Fails if the key already exists.
    fn create_commitments(
        &mut self,
        key: K,
        commitments: PerCommitmentScheme<OptionType<V>>,
    ) -> Result<(), KeyExistsError<K>>;

    /// Delete all commitments for a particular key.
    fn delete_commitments(&mut self, key: &K);
}
