//! # System Tables Pallet
//! This pallet holds logic for parsing insert statements received via indexing and
//! performing any system related on-chain state transitions
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
extern crate core;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod messages;
mod parse;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;

    use frame_support::dispatch::RawOrigin;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use on_chain_table::OnChainTable;
    use parse::{table_to_request, SystemRequest};
    use sp_core::U256;
    use sp_io::crypto::secp256k1_ecdsa_recover_compressed;
    use sp_runtime::traits::{Bounded, Hash, StaticLookup, UniqueSaturatedInto};
    use sp_runtime::{AccountId32, BoundedVec, SaturatedConversion};
    use sxt_core::permissions::{PermissionLevel, PermissionList};
    use sxt_core::tables::{extract_schema_uuid, TableIdentifier, TableName, TableNamespace};

    use super::*;
    use crate::parse::{StakingSystemRequest, SystemFieldValue, SystemRequestType};

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_tables::Config
        + pallet_session::Config
        + pallet_staking::Config
        + pallet_balances::Config
    {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    /// The last processed message
    #[pallet::storage]
    pub type MessageNonce<T: Config> = StorageValue<_, U256, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// This event is emitted whenever a message is received by Substrate from the EVM
        MessageReceived {
            /// The ethereum address of the sender
            sender: [u8; 20],
            /// The message payload received
            payload: Vec<u8>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The system table insert was missing an expected field for the supplied table identifier
        MissingExpectedField,
        /// The field expected was present, but it was not the expected type represenation
        IncorrectFieldType,
        /// Catchall error for sanity checks in parsing (i.e. request was passed to the wrong function)
        InternalError,
        /// The signature of the message was corrupt or invalid
        InvalidSignature,
        /// The address provided is not the address that signed the message
        AddressMismatch,
        /// This nonce has already been processed
        LateNonce,
        /// This nonce is too far in the future
        FutureNonce,
        /// This message is malformed
        InvalidMessageFormat,
        /// The session keys provided are malformed
        InvalidSessionKeys,
        /// The provided validator proof couldn't be verified
        InvalidValidatorProof,
    }

    impl<T: Config> Pallet<T> {
        /// Processes an insert to a system table, checking to see if there are any state
        /// modifications required onchain and applying them.
        pub fn process_system_table(
            table_id: TableIdentifier,
            oc_table: OnChainTable,
        ) -> DispatchResult {
            match table_to_request(oc_table, table_id) {
                None => Ok(()),
                Some(req) => process_request::<T>(req),
            }
        }
    }

    /// The Lock Identifier used by the staking pallet to lock funds in the balances pallet
    /// We use it to retrieve someone's staked balance
    pub(crate) const STAKING_ID: frame_support::traits::LockIdentifier = *b"staking ";

    /// Process all state changes for a given SystemRequest
    pub fn process_request<T: Config>(request: SystemRequest) -> DispatchResult {
        match request.request_type {
            SystemRequestType::Message => process_evm_message::<T>(request),
            SystemRequestType::Staking(StakingSystemRequest::Stake) => {
                process_staking::<T>(request)
            }
            SystemRequestType::Staking(StakingSystemRequest::Nominate) => {
                process_nominating::<T>(request)
            }
            SystemRequestType::Staking(StakingSystemRequest::UnstakeCancelled) => {
                process_unstake_cancelled::<T>(request)
            }
            SystemRequestType::Staking(StakingSystemRequest::UnstakeInitiated) => {
                process_unstake_initiated::<T>(request)
            }
            // SystemRequestType::ZkPayRequest => {
            //     Ok(())
            // }
            (_) => Ok(()),
        }
    }

    /// Process supplied SystemRequest as a staking request
    pub fn process_staking<T: Config>(request: SystemRequest) -> DispatchResult {
        if request.request_type != SystemRequestType::Staking(StakingSystemRequest::Stake) {
            return Err(Error::<T>::InternalError.into());
        }

        for row in request.rows() {
            if let (
                Some(SystemFieldValue::Varchar(staker)),
                Some(SystemFieldValue::Decimal(amount)),
            ) = (row.get("STAKER"), row.get("AMOUNT"))
            {
                let staker_id = sxt_core::utils::eth_address_to_substrate_account_id::<T>(staker)?;
                let staker_signer = RawOrigin::Signed(staker_id.clone());
                let amount = amount.min(&U256::from(u128::MAX)).low_u128();
                // Increase the account balance by the new stake
                let balance: u128 =
                    pallet_balances::Pallet::<T>::free_balance(&staker_id).unique_saturated_into();
                let new_total_stake = balance.saturating_add(amount);

                let staker_lookup =
                    <T as frame_system::Config>::Lookup::unlookup(staker_id.clone());

                pallet_balances::Pallet::<T>::force_set_balance(
                    RawOrigin::Root.into(),
                    staker_lookup,
                    new_total_stake.saturated_into(),
                )?;
                // The staking pallet seems to only support 64 bit balances
                let staking_balance: T::CurrencyBalance = T::CurrencyBalance::from(
                    UniqueSaturatedInto::<u64>::unique_saturated_into(amount),
                );
                pallet_staking::Pallet::<T>::bond(
                    staker_signer.clone().into(),
                    staking_balance,
                    pallet_staking::RewardDestination::Staked,
                )?;
            }
        }
        Ok(())
    }

    /// Process a Nominate SystemRequest
    pub fn process_nominating<T: Config>(request: SystemRequest) -> DispatchResult {
        if request.request_type != SystemRequestType::Staking(StakingSystemRequest::Stake) {
            return Err(Error::<T>::InternalError.into());
        }
        // List of staker ids
        let _ = request.rows().map(|row| -> DispatchResult {
            if let (
                Some(SystemFieldValue::Varchar(nominator)),
                Some(SystemFieldValue::Varchar(nodes)),
            ) = (row.get("NOMINATOR"), row.get("NODES"))
            {
                let nominator_id =
                    sxt_core::utils::eth_address_to_substrate_account_id::<T>(nominator)?;
                let nominations = sxt_core::utils::string_to_address_list::<T>(nodes.clone());
                let nominator_signer: OriginFor<T> = RawOrigin::Signed(nominator_id).into();
                pallet_staking::Pallet::<T>::nominate(nominator_signer, nominations.clone())?;
            }
            Ok(())
        });
        Ok(())
    }

    /// Parse a system request to initiate unstaking
    pub fn process_unstake_initiated<T: Config>(request: SystemRequest) -> DispatchResult {
        let _ = request.rows().map(|row| -> DispatchResult {
            if let Some(SystemFieldValue::Varchar(staker)) = row.get("STAKER") {
                let staker_id = sxt_core::utils::eth_address_to_substrate_account_id::<T>(staker)?;
                let staker_signer: OriginFor<T> = RawOrigin::Signed(staker_id.clone()).into();

                let raw_balance: u128 =
                    pallet_balances::Pallet::<T>::free_balance(staker_id).unique_saturated_into();
                let staking_balance: T::CurrencyBalance = T::CurrencyBalance::from(
                    UniqueSaturatedInto::<u64>::unique_saturated_into(raw_balance),
                );
                let _ = pallet_staking::Pallet::<T>::unbond(staker_signer, staking_balance);
            }
            Ok(())
        });
        Ok(())
    }

    /// Process a request to cancel unstakng
    pub fn process_unstake_cancelled<T: Config>(request: SystemRequest) -> DispatchResult {
        let _ = request.rows().map(|row| -> DispatchResult {
            if let Some(SystemFieldValue::Varchar(staker)) = row.get("STAKER") {
                let staker_id = sxt_core::utils::eth_address_to_substrate_account_id::<T>(staker)?;
                let staker_signer: OriginFor<T> = RawOrigin::Signed(staker_id.clone()).into();

                let raw_balance: u128 =
                    pallet_balances::Pallet::<T>::free_balance(staker_id).unique_saturated_into();
                let staking_balance: T::CurrencyBalance = T::CurrencyBalance::from(
                    UniqueSaturatedInto::<u64>::unique_saturated_into(raw_balance),
                );

                let _ = pallet_staking::Pallet::<T>::rebond(staker_signer, staking_balance);
            }
            Ok(())
        });
        Ok(())
    }

    #[pallet::storage]
    #[pallet::getter(fn last_processed_nonce)]
    pub(super) type LastProcessedNonce<T: Config> = StorageValue<_, U256, ValueQuery>;

    /// Process a message received from our EVM contract
    #[allow(clippy::comparison_chain)]
    pub fn process_evm_message<T: Config>(request: SystemRequest) -> DispatchResult {
        for row in request.rows() {
            if let (
                Some(SystemFieldValue::Varchar(sender)),
                Some(SystemFieldValue::Varchar(message)),
                Some(SystemFieldValue::Decimal(nonce)),
            ) = (row.get("SENDER"), row.get("MESSAGE"), row.get("NONCE"))
            {
                let eth_sender = sxt_core::utils::eth_address_to_substrate_account_id::<T>(sender)?;

                let nonce: U256 = *nonce;
                let expected = LastProcessedNonce::<T>::get() + 1;
                if nonce < expected {
                    return Err(Error::<T>::LateNonce.into());
                } else if nonce > expected {
                    return Err(Error::<T>::FutureNonce.into());
                }

                LastProcessedNonce::<T>::put(nonce);

                return messages::handle_message::<T>(eth_sender, message.as_bytes().to_vec());
            }
        }

        Ok(())
    }
}
