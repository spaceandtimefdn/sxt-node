//! This example demonstrates how a CommitmentStorageMap may be used in a substrate pallet.

// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

/// Generic dev_mode pallet boilerplate uncustomized for this example
pub use pallet::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet(dev_mode)]
pub mod pallet {
    use core::str;

    use curve25519_dalek::RistrettoPoint;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use proof_of_sql::base::commitment::TableCommitment;
    use proof_of_sql::proof_primitive::dory::DynamicDoryCommitment;
    use proof_of_sql_commitment_map::{
        CommitmentMap,
        CommitmentScheme,
        CommitmentStorageMapHandler,
        TableCommitmentBytes,
        TableCommitmentBytesPerCommitmentScheme,
    };
    use sxt_core::tables::TableIdentifier;

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    /// Typing for the substrate implementation of the `CommitmentMap` as a substrate
    /// `StorageDoubleMap`.
    #[pallet::storage]
    pub type CommitmentStorageMap<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        TableIdentifier,
        Blake2_128Concat,
        CommitmentScheme,
        TableCommitmentBytes,
    >;

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

            let zero_hashes = TableCommitmentBytesPerCommitmentScheme {
                hyper_kzg: Some(
                    (&TableCommitment::<RistrettoPoint>::default())
                        .try_into()
                        .unwrap(),
                ),
                dynamic_dory: Some(
                    (&TableCommitment::<DynamicDoryCommitment>::default())
                        .try_into()
                        .unwrap(),
                ),
            };

            handler
                .create_commitments(table_identifier, zero_hashes)
                .unwrap();

            Ok(())
        }
    }
}
