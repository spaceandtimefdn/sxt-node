//! # System Tables Pallet
//! This pallet holds logic for parsing insert statements received via indexing and
//! performing any system related on-chain state transitions
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
extern crate core;

use alloc::string::String;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod messages;
mod parse;
mod zkpay;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;

    use frame_support::dispatch::RawOrigin;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use itertools::Itertools;
    use on_chain_table::OnChainTable;
    use pallet_session::historical::IdentificationTuple;
    use parse::{table_to_request, SystemRequest};
    use sp_core::U256;
    use sp_runtime::traits::{StaticLookup, UniqueSaturatedInto};
    use sp_runtime::{Perbill, SaturatedConversion};
    use sp_staking::offence::{OffenceDetails, OnOffenceHandler};
    use sp_staking::SessionIndex;
    use sxt_core::permissions::{PermissionLevel, PermissionList};
    use sxt_core::tables::{extract_schema_uuid, TableIdentifier, TableName, TableNamespace};
    use sxt_core::utils::{convert_account_id, eth_address_to_substrate_account_id};

    use super::*;
    use crate::parse::{StakingSystemRequest, SystemFieldValue, SystemRequestType};

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_tables::Config
        + pallet_session::Config
        + pallet_staking::Config<CurrencyBalance = u128>
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
        /// There was an error processing an evm message
        MessageProcessingError {
            /// The error received
            error: DispatchError,
        },
        /// Emitted when a validator is chilled by the offence handler
        ValidatorForceChilled {
            /// The validator that was forcefully chilled
            validator: T::AccountId,
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
        /// Error parsing the list of nominated nodes from the message request
        ErrorParsingNominations,
        /// Empty Nomination Set
        EmptyNominationSet,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Sudo call to set the last nonce manually
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(0, 0))]
        pub fn set_last_nonce(
            origin: OriginFor<T>,
            eth_wallet: String,
            new_nonce: u64,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let eth_sender = eth_address_to_substrate_account_id::<T>(&eth_wallet)?;
            LastProcessedUserNonce::<T>::set(eth_sender, Some(U256::from(new_nonce)));
            Ok(())
        }
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
            SystemRequestType::ZkPay(_) => zkpay::process_zkpay_request::<T>(request),
            _ => Ok(()),
        }
    }

    /// Process supplied SystemRequest as a staking request
    pub fn process_staking<T: Config>(request: SystemRequest) -> DispatchResult {
        request
            .rows()
            .map(|row| -> DispatchResult {
                match (row.get("STAKER"), row.get("AMOUNT")) {
                    (
                        Some(SystemFieldValue::Bytes(staker_bytes)),
                        Some(SystemFieldValue::Decimal(amount)),
                    ) => {
                        let staker = hex::encode(staker_bytes);
                        let staker_id =
                            sxt_core::utils::eth_address_to_substrate_account_id::<T>(&staker)?;
                        let staker_signer = RawOrigin::Signed(staker_id.clone());
                        let stake_amount = amount.min(&U256::from(u128::MAX)).low_u128();
                        // Increase the account balance by the new stake
                        let balance: u128 = pallet_balances::Pallet::<T>::free_balance(&staker_id)
                            .unique_saturated_into();
                        let new_total_balance = balance.saturating_add(stake_amount);

                        let staker_lookup =
                            <T as frame_system::Config>::Lookup::unlookup(staker_id.clone());

                        pallet_balances::Pallet::<T>::force_set_balance(
                            RawOrigin::Root.into(),
                            staker_lookup,
                            new_total_balance.saturated_into(),
                        )?;

                        // If the user already had a bonded amount use bond_extra
                        if balance > 0 {
                            pallet_staking::Pallet::<T>::bond_extra(
                                staker_signer.clone().into(),
                                stake_amount,
                            )?;
                        } else {
                            pallet_staking::Pallet::<T>::bond(
                                staker_signer.clone().into(),
                                stake_amount,
                                pallet_staking::RewardDestination::Staked,
                            )?;
                        }

                        Ok(())
                    }
                    _ => Err(Error::<T>::MissingExpectedField.into()),
                }
            })
            .for_each(emit_for_error::<T>);

        Ok(())
    }

    /// Process a Nominate SystemRequest
    pub fn process_nominating<T: Config>(request: SystemRequest) -> DispatchResult {
        request
            .rows()
            .map(|row| -> DispatchResult {
                match (row.get("NOMINATOR"), row.get("NODESED25519PUBKEYS")) {
                    (
                        Some(SystemFieldValue::Bytes(nominator)),
                        Some(SystemFieldValue::Varchar(nodes)),
                    ) => {
                        // Parse the input string as a JSON list
                        let parsed = sxt_core::utils::parse_address_list_json::<T>(nodes)
                            .map_err(|_| Error::<T>::ErrorParsingNominations)?;

                        let (nominations, errors): (Vec<_>, Vec<_>) =
                            parsed.into_iter().partition_result();

                        if !errors.is_empty() {
                            log::warn!(
                                "❌ {} invalid nominations were skipped: {:?}",
                                errors.len(),
                                errors
                            );

                            errors
                                .into_iter()
                                .for_each(|e| emit_for_error::<T>(Result::<(), _>::Err(e)));
                        }

                        if nominations.is_empty() {
                            log::warn!("❌ All nominations failed to parse: {}", nodes);
                            Err(Error::<T>::EmptyNominationSet)?;
                        }

                        let nominator = hex::encode(nominator);
                        let nominator_id =
                            sxt_core::utils::eth_address_to_substrate_account_id::<T>(&nominator)?;
                        let nominator_signer: OriginFor<T> = RawOrigin::Signed(nominator_id).into();

                        pallet_staking::Pallet::<T>::nominate(nominator_signer, nominations)?;
                        Ok(())
                    }
                    _ => Err(Error::<T>::MissingExpectedField.into()),
                }
            })
            .for_each(emit_for_error::<T>);

        Ok(())
    }

    /// Parse a system request to initiate unstaking
    pub fn process_unstake_initiated<T: Config>(request: SystemRequest) -> DispatchResult {
        request
            .rows()
            .map(|row| -> DispatchResult {
                match row.get("STAKER") {
                    Some(SystemFieldValue::Bytes(staker)) => {
                        let staker = hex::encode(staker);
                        let staker_id =
                            sxt_core::utils::eth_address_to_substrate_account_id::<T>(&staker)?;
                        let staker_signer: OriginFor<T> =
                            RawOrigin::Signed(staker_id.clone()).into();

                        let raw_balance: u128 =
                            pallet_balances::Pallet::<T>::free_balance(staker_id)
                                .unique_saturated_into();
                        let staking_balance: T::CurrencyBalance = T::CurrencyBalance::from(
                            UniqueSaturatedInto::<u64>::unique_saturated_into(raw_balance),
                        );
                        pallet_staking::Pallet::<T>::unbond(staker_signer, staking_balance)
                            .map_err(|e| e.error)?;
                        Ok(())
                    }
                    _ => Err(Error::<T>::MissingExpectedField.into()),
                }
            })
            .for_each(emit_for_error::<T>);

        Ok(())
    }

    /// Process a request to cancel unstakng
    pub fn process_unstake_cancelled<T: Config>(request: SystemRequest) -> DispatchResult {
        request
            .rows()
            .map(|row| -> DispatchResult {
                match row.get("STAKER") {
                    Some(SystemFieldValue::Bytes(staker)) => {
                        let staker = hex::encode(staker);
                        let staker_id =
                            sxt_core::utils::eth_address_to_substrate_account_id::<T>(&staker)?;
                        let staker_signer: OriginFor<T> =
                            RawOrigin::Signed(staker_id.clone()).into();

                        let raw_balance: u128 =
                            pallet_balances::Pallet::<T>::free_balance(staker_id)
                                .unique_saturated_into();
                        let staking_balance: T::CurrencyBalance = T::CurrencyBalance::from(
                            UniqueSaturatedInto::<u64>::unique_saturated_into(raw_balance),
                        );

                        pallet_staking::Pallet::<T>::rebond(staker_signer, staking_balance)
                            .map_err(|e| e.error)?;
                        Ok(())
                    }
                    _ => Err(Error::<T>::MissingExpectedField.into()),
                }
            })
            .for_each(emit_for_error::<T>);
        Ok(())
    }

    fn emit_for_error<T: Config>(r: DispatchResult) {
        if let Err(error) = r {
            // Emit an event for any errors
            Pallet::<T>::deposit_event(Event::<T>::MessageProcessingError { error });
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn last_processed_nonce)]
    pub(super) type LastProcessedNonce<T: Config> = StorageValue<_, U256, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn last_processed_user_nonce)]
    pub(super) type LastProcessedUserNonce<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, U256>;

    /// Process a message received from our EVM contract
    #[allow(clippy::comparison_chain)]
    pub fn process_evm_message<T: Config>(request: SystemRequest) -> DispatchResult {
        request
            .rows()
            .map(|row| -> DispatchResult {
                match (row.get("SENDER"), row.get("BODY"), row.get("NONCE")) {
                    (
                        Some(SystemFieldValue::Bytes(sender)),
                        Some(SystemFieldValue::Bytes(body)),
                        Some(SystemFieldValue::Decimal(nonce)),
                    ) => {
                        let sender = hex::encode(sender);
                        let eth_sender = eth_address_to_substrate_account_id::<T>(&sender)?;

                        let nonce: U256 = *nonce;
                        let expected = LastProcessedUserNonce::<T>::get(&eth_sender)
                            .unwrap_or(U256::from(0))
                            + U256::from(1);
                        if nonce < expected {
                            return Err(Error::<T>::LateNonce.into());
                        } else if nonce > expected {
                            return Err(Error::<T>::FutureNonce.into());
                        }

                        LastProcessedUserNonce::<T>::set(&eth_sender, Some(nonce));

                        messages::handle_message::<T>(eth_sender, body.to_vec())?;
                        Ok(())
                    }
                    _ => Err(Error::<T>::MissingExpectedField.into()),
                }
            })
            .for_each(emit_for_error::<T>);

        Ok(())
    }

    /// A custom offence handler that chills validators when they offend
    pub struct ChillingOffenceHandler<T: pallet_staking::Config>(core::marker::PhantomData<T>);

    impl<T: pallet_staking::Config> Default for ChillingOffenceHandler<T> {
        fn default() -> Self {
            Self(Default::default())
        }
    }

    impl<Reporter, T> OnOffenceHandler<Reporter, IdentificationTuple<T>, Weight>
        for ChillingOffenceHandler<T>
    where
        T: pallet_staking::Config + pallet_session::historical::Config + crate::pallet::Config,
        T::ValidatorId: Into<T::AccountId>,
    {
        fn on_offence(
            offenders: &[OffenceDetails<Reporter, IdentificationTuple<T>>],
            _slash_fraction: &[Perbill],
            _session_index: SessionIndex,
        ) -> Weight {
            let mut weight = Weight::zero();

            for offender in offenders {
                let (validator_id, _exposure) = &offender.offender;
                let validator_account: T::AccountId = validator_id.clone().into();

                Pallet::<T>::deposit_event(Event::<T>::ValidatorForceChilled {
                    validator: validator_account.clone(),
                });

                let result =
                    pallet_staking::Pallet::<T>::chill(RawOrigin::Signed(validator_account).into());
                weight += Weight::from_parts(10_000, 0)
            }

            weight
        }
    }
}
