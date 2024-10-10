#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub use pallet::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use alloc::{str, vec::Vec};
    use frame_support::pallet_prelude::*;
    use proof_of_sql::proof_primitive::dory::PublicParameters;
    use proof_of_sql_commitment_map::{
        CommitmentMap, CommitmentScheme, CommitmentSchemeFlags, CommitmentStorageMapHandler,
        KeyExistsError, TableCommitmentBytes, TableCommitmentBytesPerCommitmentScheme,
    };
    use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
    use sxt_core::{commitments::PublicParametersBytes, tables::TableIdentifier};

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

    /// Proof of sql public parameters storage.
    #[pallet::storage]
    pub type StoredPublicParameters<T: Config> = StorageValue<_, PublicParametersBytes>;

    /// Default schemes used when committing to new tables.
    #[pallet::storage]
    pub type DefaultCommitmentSchemes<T: Config> = StorageValue<_, CommitmentSchemeFlags>;

    /// Genesis configuration struct for the commitments pallet.
    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        public_parameters_bytes: Vec<u8>,
        default_commitment_schemes: CommitmentSchemeFlags,
        _marker: PhantomData<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            let seed = "SpaceAndTime"
                .bytes()
                .chain(core::iter::repeat(0u8))
                .take(32)
                .collect::<Vec<_>>()
                .try_into()
                .expect("collection is guaranteed to contain 32 elements");
            let mut rng = ChaCha20Rng::from_seed(seed);
            let public_parameters = PublicParameters::rand(8, &mut rng);

            let public_parameters_bytes = PublicParametersBytes::try_from(public_parameters)
                .expect("default public parameters should serialize successfully")
                .data
                .into();

            let default_commitment_schemes = CommitmentSchemeFlags {
                ipa: false,
                dory: true,
            };

            GenesisConfig {
                public_parameters_bytes,
                default_commitment_schemes,
                _marker: PhantomData,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            StoredPublicParameters::<T>::put(PublicParametersBytes {
                data: self
                    .public_parameters_bytes
                    .clone()
                    .try_into()
                    .expect("genesis public parameters should be configured correctly"),
            });

            DefaultCommitmentSchemes::<T>::put(self.default_commitment_schemes);
        }
    }

    impl<T: Config> Pallet<T> {
        /// Initiates the provided table with the provided commitments in storage.
        pub fn initiate_precomputed_commitments(
            table: TableIdentifier,
            commitments: TableCommitmentBytesPerCommitmentScheme,
        ) -> Result<(), KeyExistsError<TableIdentifier>> {
            let mut handler = CommitmentStorageMapHandler::<CommitmentStorageMap<T>>::new();

            handler.create_commitments(table, commitments)
        }
    }
}
