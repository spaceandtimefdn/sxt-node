//! A Substrate pallet for managing and verifying attestations.
//!
//! This pallet allows on-chain registration and management of attestation keys,
//! as well as block-level attestations using these keys. It includes functionality
//! for verifying Ethereum-style ECDSA signatures and enforcing rules for attestation
//! registration and usage.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use frame_support::dispatch::DispatchResult;
    use frame_support::pallet_prelude::*;
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use sxt_core::attestation::{Attestation, AttestationKey};
    use sxt_core::keystore::EthereumKey;
    use sxt_core::permissions::{AttestationPalletPermission, PermissionLevel};

    use crate::weights::WeightInfo;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Type alias for storing block numbers on-chain.
    pub type BlockNumber = u32;

    /// Configuration trait for the pallet.
    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_permissions::Config + pallet_keystore::Config
    {
        /// Associated event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Weight information for extrinsics.
        type WeightInfo: WeightInfo;
    }

    /// Events emitted by the attestation pallet.
    ///
    /// These events are triggered as a result of various extrinsic calls or state changes in the pallet.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Emitted when a block is successfully attested.
        ///
        /// This event indicates that a valid attestation has been submitted and recorded on-chain.
        ///
        /// # Parameters
        /// - `block_number`: The block number that was attested.
        /// - `attestation`: The details of the attestation, including the signature and state root.
        /// - `who`: The account ID of the entity that submitted the attestation.
        BlockAttested {
            /// The number of the block that was attested.
            block_number: BlockNumber,

            /// The attestation details, including signature, public key, and state root.
            attestation: Attestation,

            /// The account ID of the attestor who submitted the attestation.
            who: T::AccountId,
        },
    }

    /// Storage for attestation keys registered on-chain.
    ///
    /// Each entry is a tuple of an account ID and its associated attestation key.
    #[pallet::storage]
    #[pallet::getter(fn validators)]
    pub type AttestationKeys<T: Config> =
        StorageValue<_, BoundedVec<(T::AccountId, AttestationKey), ConstU32<64>>, ValueQuery>;

    /// Storage for attestations recorded for specific blocks.
    ///
    /// Each entry maps a block number to a bounded vector of attestations.
    #[pallet::storage]
    #[pallet::getter(fn attestations)]
    pub type Attestations<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BlockNumber,
        BoundedVec<Attestation, ConstU32<64>>,
        ValueQuery,
    >;

    /// Errors that may occur in this pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// Error verifying ownership of an external address.
        VerificationError,
        /// Maximum number of attestation keys reached.
        MaxAttestationKeys,
        /// The public key is already registered.
        PublicKeyAlreadyRegistered,
        /// The account ID is already linked to another key.
        AccountIdAlreadyLinked,
        /// Insufficient permissions to perform the operation.
        InsufficientPermissions,
        /// Error verifying the attestation signature.
        AttestationSignatureError,
        /// Maximum attestations for the block have been recorded.
        MaxAttestationsForBlockError,
        /// Attestation already exists for the given block and key.
        AttestationAlreadyRecordedError,
        /// Cannot attest to a block that has not occurred yet.
        CannotAttestFutureBlock,
        /// Cannot attest to the current (non-finalized) block.
        CannotAttestCurrentBlock,
        /// Cannot remove a key that is not registered
        KeyNotFound,
    }

    /// Pallet extrinsics implementation.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit a block attestation.
        ///
        /// # Arguments
        /// * `block_number` - The block being attested.
        /// * `attestation` - The attestation details.
        ///
        /// # Emits
        /// * [`Event::BlockAttested`]
        ///
        /// # Errors
        /// * [`Error::CannotAttestFutureBlock`]
        /// * [`Error::CannotAttestCurrentBlock`]
        /// * [`Error::MaxAttestationsForBlockError`]
        /// * [`Error::AttestationAlreadyRecordedError`]
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::attest_block())]
        pub fn attest_block(
            origin: OriginFor<T>,
            block_number: BlockNumber,
            attestation: Attestation,
        ) -> DispatchResult {
            let who = ensure_signed(origin.clone())?;

            let current_block = frame_system::Pallet::<T>::block_number();

            ensure!(
                current_block > block_number.into(),
                Error::<T>::CannotAttestFutureBlock
            );
            ensure!(
                current_block != block_number.into(),
                Error::<T>::CannotAttestCurrentBlock
            );

            pallet_permissions::Pallet::<T>::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::AttestationPallet(AttestationPalletPermission::AttestBlock),
            )?;

            match attestation {
                Attestation::EthereumAttestation {
                    signature,
                    proposed_pub_key: attestor_pub_key,
                    ..
                } => {
                    let proposed_key = EthereumKey {
                        pub_key: attestor_pub_key,
                    };

                    pallet_keystore::Pallet::<T>::verify_ethereum_key(
                        &who,
                        &proposed_key,
                        &signature,
                    )?;

                    let mut attestations_for_block = Attestations::<T>::get(block_number);

                    Self::must_not_have_submitted_attestation(
                        &attestations_for_block,
                        &attestor_pub_key,
                    )?;

                    attestations_for_block
                        .try_push(attestation)
                        .map_err(|_| Error::<T>::MaxAttestationsForBlockError)?;

                    Attestations::<T>::insert(block_number, attestations_for_block);

                    Self::deposit_event(Event::<T>::BlockAttested {
                        block_number,
                        attestation,
                        who,
                    });
                }
            }

            Ok(())
        }
    }

    /// Utility functions for the pallet.
    impl<T: Config> Pallet<T> {
        /// Ensures that the attestor has not submitted an attestation for the given block.
        ///
        /// # Arguments
        /// * `attestations_for_block` - A bounded vector of attestations already recorded for the block.
        /// * `attestor_key` - The public key of the attestor in SEC1 format (33 bytes).
        ///
        /// # Returns
        /// * `Ok(())` if the attestor has not submitted an attestation.
        ///
        /// # Errors
        /// * [`Error::AttestationAlreadyRecordedError`] - If the attestor has already submitted an attestation.
        pub fn must_not_have_submitted_attestation(
            attestations_for_block: &BoundedVec<Attestation, ConstU32<64>>,
            attestor_key: &[u8; 33],
        ) -> DispatchResult {
            ensure!(
                !attestations_for_block.iter().any(|x| {
                    let Attestation::EthereumAttestation {
                        proposed_pub_key, ..
                    } = x;

                    proposed_pub_key == attestor_key
                }),
                Error::<T>::AttestationAlreadyRecordedError
            );

            Ok(())
        }
    }
}
