//! A keystore pallet that provides a 1:1 mapping of substrate keys to keys from other chains
//! Currently only Ethereum style ECDSA keys are supported

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

/// A pallet that allows users to link their account_ids with off chain addresses using cryptographic proofs.
///
/// Supported keys:
///     - Ethereum ECDSA
#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use frame_support::dispatch::DispatchResult;
    use frame_support::pallet_prelude::*;
    use frame_support::Blake2_128Concat;
    use frame_system::pallet_prelude::*;
    use sxt_core::attestation::{verify_eth_signature, EthereumSignature, RegisterExternalAddress};
    use sxt_core::keystore::{EthereumKey, UnregisterExternalAddress, UserKeystore};

    use crate::weights::WeightInfo;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Associated event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Weight information for extrinsics.
        type WeightInfo: WeightInfo;
    }

    /// Storage for keys registered on-chain.
    ///
    /// Maps an account ID to its associated `KeystoreValue`.
    #[pallet::storage]
    #[pallet::getter(fn keys)]
    pub type Keys<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, UserKeystore, OptionQuery>;

    /// Errors that may occur in the keystore pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// There was an error verifying the key registration
        VerificationError,
        /// This account id has already been used to register a key
        AccountAlreadyRegistered,
        /// The key that was requested to be removed could not be found in storage
        KeyNotFound,
        /// No keys have been registered for this account id
        NoKeysRegistered,
        /// No ethereum key registererd
        NoEthereumKeyRegistered,
        /// An ethereum key has already been registered, to register a new one you must deregister the old one
        EthereumKeyAlreadyRegistered,
        /// The verification of the signature was not successful
        SignatureVerificationFailed,
        /// The key provided does not match what is stored on chain
        KeyMismatch,
    }

    /// Events emitted by the keystore pallet.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A key has been successfully registered.
        EthereumKeyRegistered {
            /// The AccountId being associated with a key
            who: T::AccountId,
            /// The key registered with this account
            key: EthereumKey,
        },
        /// A key has been removed.
        EthereumKeyRemoved {
            /// A key was removed for this account id
            who: T::AccountId,
        },
    }

    /// Pallet extrinsics implementation.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a key.
        ///
        /// # Arguments
        /// * `who` - The account ID to associate with the key.
        /// * `registration` - The external key registration details.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_key())]
        pub fn register_key(
            origin: OriginFor<T>,
            who: T::AccountId,
            registration: RegisterExternalAddress,
        ) -> DispatchResult {
            ensure_root(origin)?;

            match registration {
                RegisterExternalAddress::EthereumAddress {
                    signature,
                    proposed_pub_key,
                    address20,
                } => {
                    let msg = who.encode();
                    verify_eth_signature(&msg, &signature, &proposed_pub_key)
                        .map_err(|_| Error::<T>::VerificationError)?;

                    let new_key = EthereumKey {
                        pub_key: proposed_pub_key,
                        address20,
                    };
                    Self::add_ethereum_key(who.clone(), new_key.clone())?;
                    Self::deposit_event(Event::<T>::EthereumKeyRegistered { who, key: new_key });
                }
            }

            Ok(())
        }

        /// Remove a registered key.
        ///
        /// # Arguments
        /// * `who` - The account ID associated with the key to remove.
        /// * `key` - The type of key to unregister
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::unregister_key())]
        pub fn unregister_key(
            origin: OriginFor<T>,
            who: T::AccountId,
            key: UnregisterExternalAddress,
        ) -> DispatchResult {
            ensure_root(origin)?;

            match key {
                UnregisterExternalAddress::EthereumAddress => {
                    Self::remove_ethereum_key(who.clone())?;
                    Self::deposit_event(Event::<T>::EthereumKeyRemoved { who });
                }
            }

            Ok(())
        }
    }

    /// Utility functions for the keystore pallet.
    impl<T: Config> Pallet<T> {
        /// Add a new ethereum key for an account.
        fn add_ethereum_key(who: T::AccountId, key: EthereumKey) -> DispatchResult {
            let user_keystore = Keys::<T>::get(who.clone());

            if user_keystore.is_none() {
                let user_keystore = UserKeystore { eth_key: Some(key) };
                Keys::<T>::insert(who, user_keystore);
                return Ok(());
            }

            let user_keystore = user_keystore.unwrap();

            ensure!(
                user_keystore.eth_key.is_none(),
                Error::<T>::EthereumKeyAlreadyRegistered
            );

            let new_user_keystore = user_keystore.with_eth_key(Some(key));

            Keys::<T>::insert(who, new_user_keystore);
            Ok(())
        }

        /// Remove the existing ethereum key for an account.
        fn remove_ethereum_key(who: T::AccountId) -> DispatchResult {
            let user_keystore = Keys::<T>::get(&who);

            // Ensure that the keystore exists
            ensure!(user_keystore.is_some(), Error::<T>::NoKeysRegistered);

            let mut user_keystore = user_keystore.unwrap();

            // Ensure that an Ethereum key is actually registered before removing it
            ensure!(
                user_keystore.eth_key.is_some(),
                Error::<T>::NoEthereumKeyRegistered
            );

            // Remove the Ethereum key
            user_keystore.eth_key = None;

            // If the keystore is empty after removal, we can remove the entry entirely
            Keys::<T>::insert(&who, user_keystore);

            Ok(())
        }

        /// Verifies the Ethereum key and its associated signature for a given account.
        ///
        /// This function checks if the provided `EthereumKey` matches the one stored on-chain
        /// for the specified account (`who`) and verifies the validity of the provided signature.
        ///
        /// # Arguments
        ///
        /// * `who` - The account ID whose Ethereum key is being verified.
        /// * `key` - The Ethereum key (public key) provided for verification.
        /// * `signature` - The cryptographic signature to verify against the account ID and key.
        ///
        /// # Returns
        ///
        /// * `Ok(())` - If the Ethereum key and signature are successfully verified.
        /// * `Err` - Returns an appropriate error if:
        ///     - The account has no associated keystore (`KeyNotFound`).
        ///     - No Ethereum key is registered for the account (`KeyNotFound`).
        ///     - The provided key does not match the stored key (`KeyMismatch`).
        ///     - The signature verification fails (`SignatureVerificationFailed`).
        ///
        /// # Errors
        ///
        /// This function can return the following errors:
        /// * [`Error::KeyNotFound`] - If the account does not have a keystore or the keystore
        ///   does not contain an Ethereum key.
        /// * [`Error::KeyMismatch`] - If the provided Ethereum key does not match the stored key.
        /// * [`Error::SignatureVerificationFailed`] - If the cryptographic signature is invalid.
        ///
        /// # Security
        ///
        /// The `verify_eth_signature` function is used to ensure that the signature matches
        /// the provided public key and account ID. It is critical to ensure the signature
        /// and key generation are secure and follow Ethereum's ECDSA standards.
        ///
        /// # Notes
        ///
        /// - This function assumes that the `EthereumKey` is properly formatted and adheres
        ///   to Ethereum's cryptographic standards.
        pub fn verify_ethereum_key(
            who: &T::AccountId,
            key: &EthereumKey,
            signature: &EthereumSignature,
        ) -> DispatchResult {
            let msg = who.encode();

            Self::verify_ethereum_msg(who, &msg, key, signature)
        }

        /// verify an ethereum message by checking its signature
        pub fn verify_ethereum_msg(
            who: &T::AccountId,
            msg: &[u8],
            key: &EthereumKey,
            signature: &EthereumSignature,
        ) -> DispatchResult {
            let keystore = Keys::<T>::get(who).ok_or(Error::<T>::KeyNotFound)?;

            let stored_key = keystore.eth_key.ok_or(Error::<T>::KeyNotFound)?;
            let stored_key = stored_key.pub_key;
            let EthereumKey { pub_key, .. } = key;

            ensure!(stored_key == *pub_key, Error::<T>::KeyMismatch);

            verify_eth_signature(msg, signature, pub_key)
                .map_err(|_| Error::<T>::SignatureVerificationFailed)?;

            Ok(())
        }
    }
}
