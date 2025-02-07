//! # Parsing Pallet
//! TODO
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

mod parse;
#[cfg(test)]
mod tests;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use alloc::string::String;
    use alloc::vec::Vec;

    use codec::Decode;
    use frame_support::dispatch::RawOrigin;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use hex::FromHex;
    use on_chain_table::OnChainTable;
    use pallet_staking::ValidatorPrefs;
    use parse::{table_to_request, SystemRequest};
    use sp_core::U256;
    use sp_runtime::traits::{Bounded, Hash, StaticLookup, UniqueSaturatedInto};
    use sp_runtime::{AccountId32, BoundedVec, SaturatedConversion};
    use sxt_core::tables::{TableIdentifier, TableName, TableNamespace};

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
        + pallet_validators::Config
    {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// The system table insert was missing an expected field for the supplied table identifier
        MissingExpectedField,
        /// The field expected was present, but it was not the expected type represenation
        IncorrectFieldType,
        /// Catchall error for sanity checks in parsing (i.e. request was passed to the wrong function)
        InternalError,
    }

    impl<T: Config> Pallet<T> {
        /// TODO Docs
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
            SystemRequestType::StakingRequest(StakingSystemRequest::Stake) => {
                process_staking::<T>(request)
            }
            SystemRequestType::StakingRequest(StakingSystemRequest::Nominate) => {
                process_nominating::<T>(request)
            }
            SystemRequestType::StakingRequest(StakingSystemRequest::UnstakeCancelled) => {
                process_unstake_cancelled::<T>(request)
            }
            SystemRequestType::StakingRequest(StakingSystemRequest::UnstakeInitiated) => {
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
        if request.request_type != SystemRequestType::StakingRequest(StakingSystemRequest::Stake) {
            return Err(Error::<T>::InternalError.into());
        }

        for row in request.rows() {
            if let (
                Some(SystemFieldValue::Varchar(staker)),
                Some(SystemFieldValue::Varchar(nodes)),
                Some(SystemFieldValue::Decimal(amount)),
            ) = (row.get("STAKER"), row.get("NODES"), row.get("AMOUNT"))
            {
                let staker_id = eth_address_to_substrate_account_id::<T>(staker)?;
                let staker_signer = RawOrigin::Signed(staker_id.clone());
                let nominations = string_to_address_list::<T>(nodes.clone());
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
                pallet_staking::Pallet::<T>::nominate(staker_signer.clone().into(), nominations)?;
            }
        }
        Ok(())
    }

    /// Process a Nominate SystemRequest
    pub fn process_nominating<T: Config>(request: SystemRequest) -> DispatchResult {
        if request.request_type != SystemRequestType::StakingRequest(StakingSystemRequest::Stake) {
            return Err(Error::<T>::InternalError.into());
        }
        // List of staker ids
        let _ = request.rows().map(|row| -> DispatchResult {
            if let (
                Some(SystemFieldValue::Varchar(nominator)),
                Some(SystemFieldValue::Varchar(nodes)),
            ) = (row.get("NOMINATOR"), row.get("NODES"))
            {
                let nominator_id = eth_address_to_substrate_account_id::<T>(nominator)?;
                let nominations = string_to_address_list::<T>(nodes.clone());
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
                let staker_id = eth_address_to_substrate_account_id::<T>(staker)?;
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

    /// Process a request to cancel unstaking
    pub fn process_unstake_cancelled<T: Config>(request: SystemRequest) -> DispatchResult {
        let _ = request.rows().map(|row| -> DispatchResult {
            if let Some(SystemFieldValue::Varchar(staker)) = row.get("STAKER") {
                let staker_id = eth_address_to_substrate_account_id::<T>(staker)?;
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

    pub fn string_to_address_list<T: frame_system::Config>(
        address_list: String,
    ) -> Vec<<T::Lookup as StaticLookup>::Source> {
        address_list
            .split(',')
            .filter_map(|s| {
                Some(<T as frame_system::Config>::Lookup::unlookup(
                    eth_address_to_substrate_account_id::<T>(s.trim()).ok()?,
                ))
            })
            .collect()
    }

    /// This function takes a Ethereum Wallet Address and transforms it into a Substrate
    /// compatible AccountId
    pub fn eth_address_to_substrate_account_id<T: frame_system::Config>(
        eth_addr_hex: &str,
    ) -> Result<T::AccountId, DispatchError> {
        // Strip optional "0x" prefix, decode the remaining hex.
        let hex_str = eth_addr_hex.trim_start_matches("0x");
        let raw_addr = <[u8; 20]>::from_hex(hex_str).map_err(|_| "Invalid hex address")?;

        // Pad a 32-byte array with zeros on the left, copy the 20 bytes at the end.
        let mut data = [0u8; 32];
        data[12..32].copy_from_slice(&raw_addr);
        convert_account_id::<T>(sp_runtime::AccountId32::from(data))
    }

    pub fn convert_account_id<T: frame_system::Config>(
        account_id32: AccountId32,
    ) -> Result<T::AccountId, DispatchError>
    where
        T::AccountId: Decode,
    {
        // Use fully qualified syntax to decode `AccountId32` into `T::AccountId`
        T::AccountId::decode(&mut <AccountId32 as AsRef<[u8]>>::as_ref(&account_id32))
            .map_err(|_| DispatchError::Other("Failed to decode AccountId32 into T::AccountId"))
    }
}
