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

pub mod weights;
pub use weights::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use frame_support::dispatch::DispatchResult;
    use frame_support::pallet_prelude::*;
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use sxt_core::attestation::{
        verify_eth_signature,
        Attestation,
        AttestationKey,
        EthereumSignature,
        RegisterExternalAddress,
    };

    use crate::weights::WeightInfo;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Type alias for storing block numbers on-chain.
    pub type BlockNumber = u32;

    /// Configuration trait for the pallet.
    #[pallet::config]
    pub trait Config: frame_system::Config {
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
        /// Register an external attestation key.
        ///
        /// # Arguments
        /// * `who` - The account ID associated with the attestation key.
        /// * `registration` - The external key registration details.
        ///
        /// # Emits
        /// * [`Event::BlockAttested`]
        ///
        /// # Errors
        /// * [`Error::VerificationError`]
        /// * [`Error::PublicKeyAlreadyRegistered`]
        /// * [`Error::AccountIdAlreadyLinked`]
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_attestation_key())]
        pub fn register_attestation_key(
            origin: OriginFor<T>,
            who: T::AccountId,
            registration: RegisterExternalAddress,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Self::try_validate_attestation_key_registration(who, registration)
        }

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
        #[pallet::weight(T::WeightInfo::attest_block())]
        pub fn attest_block(
            origin: OriginFor<T>,
            block_number: BlockNumber,
            attestation: Attestation,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let current_block = frame_system::Pallet::<T>::block_number();

            ensure!(
                current_block > block_number.into(),
                Error::<T>::CannotAttestFutureBlock
            );
            ensure!(
                current_block != block_number.into(),
                Error::<T>::CannotAttestCurrentBlock
            );

            match attestation {
                Attestation::EthereumAttestation {
                    signature,
                    proposed_pub_key: attestor_pub_key,
                    ..
                } => {
                    let proposed_attestation_key = AttestationKey::EthereumKey {
                        pub_key: attestor_pub_key,
                    };

                    Self::must_verify_eth_signature(&who, &signature, &attestor_pub_key)?;
                    Self::must_be_registered_attestor(&who, &proposed_attestation_key)?;

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

        /// Remove an attestation key.
        ///
        /// This extrinsic allows the root (sudo) origin to remove an attestation key.
        ///
        /// # Arguments
        /// * `who` - The account ID associated with the attestation key to be removed.
        /// * `key` - The attestation key to be removed.
        ///
        /// # Errors
        /// * [`Error::InsufficientPermissions`] - If the caller does not have sufficient permissions.
        /// * [`Error::PublicKeyAlreadyRegistered`] - If the key is not found in the storage.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::remove_attestation_key())]
        pub fn remove_attestation_key(
            origin: OriginFor<T>,
            who: T::AccountId,
            key: AttestationKey,
        ) -> DispatchResult {
            // Ensure the caller has root (sudo) access
            ensure_root(origin)?;

            // Fetch the list of attestation keys
            let mut attestation_keys = AttestationKeys::<T>::get();

            // Find the index of the key to be removed
            if let Some(index) = attestation_keys
                .iter()
                .position(|(account_id, stored_key)| account_id == &who && stored_key == &key)
            {
                // Remove the key from the list
                attestation_keys.remove(index);

                // Update storage
                AttestationKeys::<T>::put(attestation_keys);

                Ok(())
            } else {
                // Return an error if the key is not found
                Err(Error::<T>::KeyNotFound.into())
            }
        }
    }

    /// Utility functions for the pallet.
    impl<T: Config> Pallet<T> {
        /// Verifies an Ethereum signature.
        pub fn must_verify_eth_signature(
            who: &T::AccountId,
            signature: &EthereumSignature,
            proposed_pub_key: &[u8; 33],
        ) -> DispatchResult {
            let msg = who.encode();
            verify_eth_signature(&msg, signature, proposed_pub_key)
                .map_err(|_| Error::<T>::AttestationSignatureError)?;

            Ok(())
        }

        /// Ensures the given attestation key and account ID are registered.
        pub fn must_be_registered_attestor(
            who: &T::AccountId,
            attestation_key: &AttestationKey,
        ) -> DispatchResult {
            let keys = AttestationKeys::<T>::get();

            ensure!(
                keys.contains(&(who.clone(), attestation_key.clone())),
                Error::<T>::InsufficientPermissions
            );

            Ok(())
        }

        /// Validates and registers an external attestation key.
        pub fn try_validate_attestation_key_registration(
            id: T::AccountId,
            registration: RegisterExternalAddress,
        ) -> DispatchResult {
            match registration {
                RegisterExternalAddress::EthereumAddress {
                    signature,
                    proposed_pub_key,
                } => {
                    let msg = id.encode();
                    Self::try_register_ethereum_address(&id, &msg, &signature, &proposed_pub_key)
                }
            }
        }

        /// Attempts to register an Ethereum address as an attestation key.
        ///
        /// # Arguments
        /// * `id` - The account ID associated with the attestation key.
        /// * `msg` - The message that was signed (e.g., the account ID encoded as bytes).
        /// * `signature` - The Ethereum-style ECDSA signature.
        /// * `pub_key` - The proposed public key in SEC1 format (33 bytes).
        ///
        /// # Returns
        /// * `Ok(())` if the registration is successful.
        ///
        /// # Errors
        /// * [`Error::VerificationError`] - If the signature cannot be verified.
        /// * [`Error::AccountIdAlreadyLinked`] - If the account ID is already linked to a key.
        /// * [`Error::PublicKeyAlreadyRegistered`] - If the key is already registered.
        /// * [`Error::MaxAttestationKeys`] - If the maximum number of keys is reached.
        pub fn try_register_ethereum_address(
            id: &T::AccountId,
            msg: &[u8],
            signature: &EthereumSignature,
            pub_key: &[u8; 33],
        ) -> DispatchResult {
            // Verify the signature.
            verify_eth_signature(msg, signature, pub_key)
                .map_err(|_| Error::<T>::VerificationError)?;

            // Construct a new attestation key and attempt to add it.
            let new_key = AttestationKey::EthereumKey { pub_key: *pub_key };
            Self::try_add_attestation_key(id.clone(), new_key)
        }

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
                    if let Attestation::EthereumAttestation {
                        proposed_pub_key, ..
                    } = x
                    {
                        proposed_pub_key == attestor_key
                    } else {
                        false
                    }
                }),
                Error::<T>::AttestationAlreadyRecordedError
            );

            Ok(())
        }

        /// Attempts to add a new attestation key to the storage.
        ///
        /// This function enforces the following rules:
        /// 1. An account ID can only be linked to one attestation key.
        /// 2. An attestation key must not already be registered to another account.
        /// 3. The total number of attestation keys must not exceed the maximum limit.
        ///
        /// # Arguments
        /// * `id` - The account ID to associate with the new attestation key.
        /// * `new_key` - The new attestation key to register.
        ///
        /// # Returns
        /// * `Ok(())` if the key is successfully added.
        ///
        /// # Errors
        /// * [`Error::AccountIdAlreadyLinked`] - If the account ID is already associated with a key.
        /// * [`Error::PublicKeyAlreadyRegistered`] - If the key is already registered to another account.
        /// * [`Error::MaxAttestationKeys`] - If the maximum number of keys is reached.
        pub fn try_add_attestation_key(
            id: T::AccountId,
            new_key: AttestationKey,
        ) -> DispatchResult {
            // Ensure the account ID is not already linked to another key.
            ensure!(
                !Self::is_account_id_used(&id),
                Error::<T>::AccountIdAlreadyLinked
            );

            // Ensure the key is not already registered.
            ensure!(
                !Self::is_attestation_key_registered(&new_key),
                Error::<T>::PublicKeyAlreadyRegistered
            );

            // Get the current list of attestation keys.
            let mut attestation_keys = AttestationKeys::<T>::get();

            // Attempt to add the new key, ensuring the maximum limit is not exceeded.
            attestation_keys
                .try_push((id.clone(), new_key))
                .map_err(|_| Error::<T>::MaxAttestationKeys)?;

            // Update the storage with the new list of keys.
            AttestationKeys::<T>::put(attestation_keys);

            Ok(())
        }

        /// Return true if the attestation key is already registered
        pub fn is_attestation_key_registered(addr: &AttestationKey) -> bool {
            AttestationKeys::<T>::get()
                .iter()
                .any(|(_, key)| key == addr)
        }

        /// Return true if the account id is already in use
        pub fn is_account_id_used(id: &T::AccountId) -> bool {
            AttestationKeys::<T>::get()
                .iter()
                .any(|(account_id, _)| account_id == id)
        }
    }
}
