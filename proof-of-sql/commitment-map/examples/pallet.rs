//! This example demonstrates how a CommitmentStorageMap may be used in a substrate pallet.

// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

/// Generic dev_mode pallet boilerplate uncustomized for this example
pub use pallet::*;

#[frame_support::pallet(dev_mode)]
pub mod pallet {
    use super::*;
    use core::str;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use proof_of_sql_commitment_map::{
        CommitmentHash, CommitmentHashType, CommitmentMap, CommitmentStorageMapHandler,
        CommitmentStorageMapKey, PerCommitmentScheme, TypedCommitmentHash,
    };
    use sp_core::H256;
    use sxt_core::tables::TableIdentifier;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    /// Typing for the substrate implementation of the `CommitmentMap` as a substrate `StorageMap`.
    #[pallet::storage]
    pub type CommitmentStorageMap<T: Config> =
        StorageMap<_, Blake2_128Concat, CommitmentStorageMapKey, CommitmentHash>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sample call leveraging the commitment storage map
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn set_commitment_hash_to_zero(
            _: OriginFor<T>,
            table_identifier: TableIdentifier,
        ) -> DispatchResult {
            // Instantiate a handler for accessing the `CommitmentMap` methods.
            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();

            let zero_hashes = PerCommitmentScheme::<CommitmentHashType> {
                ipa: Some(TypedCommitmentHash::new(H256::zero())),
                dory: Some(TypedCommitmentHash::new(H256::zero())),
            };

            handler
                .create_commitments(table_identifier, zero_hashes)
                .unwrap();

            Ok(())
        }
    }
}
