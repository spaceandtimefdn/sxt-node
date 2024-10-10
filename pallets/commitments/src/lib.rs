#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use proof_of_sql_commitment_map::{
        generic_over_commitment::{ConcreteType, OptionType},
        CommitmentMap, CommitmentScheme, CommitmentStorageMapHandler, KeyExistsError,
        PerCommitmentScheme, TableCommitmentBytes,
    };
    use sxt_core::tables::TableIdentifier;

    /// Commitment pallet, providing methods for pallet calls
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The commitment pallet's configuration trait.
    #[pallet::config]
    pub trait Config: frame_system::Config {}

    /// Mapping of tables to their current commitments, stored on chain.
    #[pallet::storage]
    #[pallet::getter(fn table_commitment)]
    pub type CommitmentStorageMap<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TableIdentifier,
        Blake2_128Concat,
        CommitmentScheme,
        TableCommitmentBytes,
    >;

    impl<T: Config> Pallet<T> {
        /// Initiates the provided table with the provided commitments in storage.
        pub fn initiate_precomputed_commitments(
            table: TableIdentifier,
            commitments: PerCommitmentScheme<OptionType<ConcreteType<TableCommitmentBytes>>>,
        ) -> Result<(), KeyExistsError<TableIdentifier>> {
            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();

            handler.create_commitments(table, commitments)
        }
    }
}
