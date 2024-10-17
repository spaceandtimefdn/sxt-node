use core::fmt::Debug;

use crate::generic_over_commitment::{GenericOverCommitment, OptionType};
use crate::{
    AnyCommitmentScheme,
    CommitmentMap,
    CommitmentScheme,
    CommitmentSchemeFlags,
    CommitmentSchemesMismatchError,
    KeyExistsError,
    PerCommitmentScheme,
};

/// Private abstraction for mappings of keys and commitment schemes to values of type `V`.
///
/// This trait allows for easy implementation of [`CommitmentMap`].
/// Intended for maps that don't have optimizations around setting multiple values simultaneously.
///
/// This trait should remain crate-private.
/// Only the key-atomic [`CommitmentMap`] operations should be available outside of the crate.
pub trait CommitmentMapImplementor<K, V: GenericOverCommitment> {
    /// Returns `true` if this map contains this key and scheme.
    fn has_key_and_scheme_impl(&self, key: &K, scheme: &CommitmentScheme) -> bool;

    /// Returns the commitment data for a particular key and any scheme.
    fn get_commitment_for_any_scheme_impl(
        &self,
        key: &K,
        scheme: &CommitmentScheme,
    ) -> AnyCommitmentScheme<OptionType<V>>;

    /// Set the commitment in this map for a particular key and any scheme.
    fn set_commitment_for_any_scheme_impl(&mut self, key: K, commitment: AnyCommitmentScheme<V>);

    /// Delete commitment for a particular key and any scheme.
    fn delete_commitment_for_any_scheme_impl(
        &mut self,
        key: &K,
        commitment_scheme: &CommitmentScheme,
    );
}

impl<M: CommitmentMapImplementor<K, V>, K: Clone + Debug, V: GenericOverCommitment>
    CommitmentMap<K, V> for M
{
    fn has_key_and_scheme(&self, key: &K, scheme: &CommitmentScheme) -> bool {
        self.has_key_and_scheme_impl(key, scheme)
    }

    fn schemes_for_key(&self, key: &K) -> CommitmentSchemeFlags {
        CommitmentSchemeFlags::all()
            .into_iter()
            .filter(|scheme| self.has_key_and_scheme_impl(key, scheme))
            .collect()
    }

    fn has_key(&self, key: &K) -> bool {
        self.schemes_for_key(key).into_iter().count() > 0
    }

    fn get_commitments(&self, key: &K) -> PerCommitmentScheme<OptionType<V>> {
        CommitmentSchemeFlags::all()
            .into_iter()
            .flat_map(|scheme| {
                self.get_commitment_for_any_scheme_impl(key, &scheme)
                    .transpose_option()
            })
            .collect()
    }

    fn update_commitments(
        &mut self,
        key: K,
        commitments: PerCommitmentScheme<OptionType<V>>,
    ) -> Result<(), CommitmentSchemesMismatchError> {
        let original_schemes = self.schemes_for_key(&key);

        let new_schemes = commitments.to_flags();

        if original_schemes != new_schemes {
            return Err(CommitmentSchemesMismatchError {
                original_schemes,
                new_schemes,
            });
        }

        commitments
            .into_flat_iter()
            .for_each(|c| self.set_commitment_for_any_scheme_impl(key.clone(), c));
        Ok(())
    }

    fn create_commitments(
        &mut self,
        key: K,
        commitments: PerCommitmentScheme<OptionType<V>>,
    ) -> Result<(), KeyExistsError<K>> {
        if self.has_key(&key) {
            return Err(KeyExistsError { key });
        }

        commitments
            .into_flat_iter()
            .for_each(|c| self.set_commitment_for_any_scheme_impl(key.clone(), c));
        Ok(())
    }

    fn delete_commitments(&mut self, key: &K) {
        CommitmentSchemeFlags::all()
            .into_iter()
            .for_each(|c| self.delete_commitment_for_any_scheme_impl(key, &c));
    }
}
