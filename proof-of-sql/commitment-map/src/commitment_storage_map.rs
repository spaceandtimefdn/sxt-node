//! Contains implementation of `CommitmentMap` for substrate's `StorageMap`.

use crate::{
    commitment_map_implementor::CommitmentMapImplementor,
    commitment_scheme::{AnyCommitmentScheme, CommitmentScheme},
    generic_over_commitment::GenericOverCommitment,
};
use core::marker::PhantomData;
use frame_support::storage::StorageMap;
use proof_of_sql::base::commitment::Commitment;
use sp_core::H256;
use sxt_core::tables::TableIdentifier;

/// Key used by substrate [`CommitmentMap`] implementation.
pub type CommitmentStorageMapKey = (TableIdentifier, CommitmentScheme);

/// Value used by substrate [`CommitmentMap`] implementation.
pub type CommitmentHash = H256;

/// Wrapper around [`CommitmentHash`] made generic over a commitment type.
pub struct TypedCommitmentHash<C: Commitment> {
    commitment_hash: CommitmentHash,
    phantom: PhantomData<C>,
}

impl<C: Commitment> TypedCommitmentHash<C> {
    /// Construct a new [`TypedCommitmentHash`]
    pub fn new(commitment_hash: H256) -> Self {
        TypedCommitmentHash {
            commitment_hash,
            phantom: PhantomData,
        }
    }

    /// Immutable accessor for the internal commitment hash.
    pub fn get(&self) -> &H256 {
        &self.commitment_hash
    }
}

/// Concrete type associated with `TypedCommitmentHash<C: Commitment>`.
pub struct CommitmentHashType;

impl GenericOverCommitment for CommitmentHashType {
    type WithCommitment<C: Commitment> = TypedCommitmentHash<C>;
}

/// Instantiable type leveraging a substrate [`StorageMap`] for commitments.
///
/// Implements [`CommitmentMap`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitmentStorageMapHandler<S>(PhantomData<S>)
where
    S: StorageMap<CommitmentStorageMapKey, CommitmentHash, Query = Option<CommitmentHash>>;

impl<S> CommitmentStorageMapHandler<S>
where
    S: StorageMap<CommitmentStorageMapKey, CommitmentHash, Query = Option<CommitmentHash>>,
{
    /// Construct a new [`CommitmentStorageMapHandler`].
    pub fn new() -> Self {
        CommitmentStorageMapHandler(PhantomData)
    }
}

impl<S> CommitmentMapImplementor<TableIdentifier, CommitmentHashType>
    for CommitmentStorageMapHandler<S>
where
    S: StorageMap<CommitmentStorageMapKey, CommitmentHash, Query = Option<CommitmentHash>>,
{
    fn has_key_and_scheme_impl(&self, key: &TableIdentifier, scheme: &CommitmentScheme) -> bool {
        S::contains_key((key, scheme))
    }

    fn set_commitment_for_any_scheme_impl(
        &mut self,
        key: TableIdentifier,
        commitment: AnyCommitmentScheme<CommitmentHashType>,
    ) {
        let scheme = commitment.to_scheme();

        let commitment_hash = match &commitment {
            AnyCommitmentScheme::Ipa(typed_hash) => typed_hash.get(),
            AnyCommitmentScheme::Dory(typed_hash) => typed_hash.get(),
        };

        S::insert((key, scheme), commitment_hash);
    }

    fn delete_commitment_for_any_scheme_impl(
        &mut self,
        key: &TableIdentifier,
        scheme: &CommitmentScheme,
    ) {
        S::remove((key, scheme));
    }
}
