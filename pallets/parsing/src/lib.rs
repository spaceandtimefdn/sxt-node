//! # Parsing Pallet
//! TODO
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

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
    use sp_runtime::traits::{Bounded, Hash, StaticLookup, UniqueSaturatedInto};
    use sp_runtime::{AccountId32, BoundedVec, SaturatedConversion};
    use sxt_core::tables::{TableIdentifier, TableName, TableNamespace};

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_tables::Config
        + pallet_staking::Config
        + pallet_balances::Config
    {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {}

    impl<T: Config> Pallet<T> {
        pub fn parse_staking(table_id: TableIdentifier, oc_table: OnChainTable) -> DispatchResult {
            let stake_table = TableName::try_from("STAKED".as_bytes().to_vec()).unwrap();
            let nominate_table = TableName::try_from("NOMINATE".as_bytes().to_vec()).unwrap();
            let unstakeinitiated_table =
                TableName::try_from("UNSTAKEINITIATED".as_bytes().to_vec()).unwrap();
            let noderegistered_table =
                TableName::try_from("NODEREGISTERED".as_bytes().to_vec()).unwrap();
            let unstakecancelled_table =
                TableName::try_from("UNSTAKECANCELLED".as_bytes().to_vec()).unwrap();

            if table_id.name == stake_table {
                parse_stake::<T>(oc_table);
            } else if table_id.name == nominate_table {
                parse_nominate::<T>(oc_table);
            } else if table_id.name == unstakeinitiated_table {
                parse_unstakeinitiated::<T>(oc_table);
            } else if table_id.name == noderegistered_table {
                parse_noderegistered::<T>(oc_table);
            } else if table_id.name == unstakecancelled_table {
                parse_unstakecancelled::<T>(oc_table);
            }
            // Return a successful `DispatchResult`
            Ok(())
        }
    }

    /// The Lock Identifier used by the staking pallet to lock funds in the balances paller
    /// We use it to retrieve someone's staked balance
    pub(crate) const STAKING_ID: frame_support::traits::LockIdentifier = *b"staking ";

    pub struct StakeRequest<T: frame_system::Config> {
        pub staker_id: T::AccountId,
        pub amount: u128, // Our substrate implementation uses u128 for balances
        pub nominations: Vec<<T::Lookup as StaticLookup>::Source>,
    }

    pub fn parse_stake<T>(oc_table: OnChainTable) -> DispatchResult
    where
        T: crate::Config,
    {
        // List of staker ids
        let staker_id = oc_table.get_varchars_by_column("STAKER");
        // list of target nodes as a comma separated string (?)
        let target_nodes = oc_table.get_varchars_by_column("NODES");
        // List of stake amounts corresponding to each stake request
        let stake_amount = oc_table.get_decimal_by_column("AMOUNT");

        match (staker_id, target_nodes, stake_amount) {
            (Some(ids), Some(nodes), Some(stake)) => {
                let stake_requests: Vec<StakeRequest<T>> = ids
                    .iter()
                    .enumerate()
                    .filter_map(|(i, id)| {
                        Some(StakeRequest {
                            staker_id: eth_address_to_substrate_account_id::<T>(id).ok()?,
                            amount: stake[i].min(u128::MAX.into()).low_u128(), // Take the lower number of either u128 max value or the original value and then take just the 128 LSB
                            nominations: nodes[i]
                                .split(',')
                                .filter_map(|s| {
                                    Some(<T as frame_system::Config>::Lookup::unlookup(
                                        eth_address_to_substrate_account_id::<T>(s.trim()).ok()?,
                                    ))
                                })
                                .collect(),
                        })
                    })
                    .collect();

                // Process each stake request
                for request in stake_requests {
                    let staker_signer = RawOrigin::Signed(request.staker_id.clone());

                    // Increase the account balance by the new stake
                    let balance: u128 =
                        pallet_balances::Pallet::<T>::free_balance(&request.staker_id)
                            .unique_saturated_into();
                    let new_total_stake = balance.saturating_add(request.amount);

                    // let system_signer = RawOrigin::Root;
                    // let system_origin: OriginFor<T> =
                    //     system_signer.into();
                    // let staker_origin: OriginFor<T> =
                    //     staker_signer.clone().into();
                    let staker_lookup =
                        <T as frame_system::Config>::Lookup::unlookup(request.staker_id);

                    pallet_balances::Pallet::<T>::force_set_balance(
                        RawOrigin::Root.into(),
                        staker_lookup,
                        new_total_stake.saturated_into(),
                    )?;
                    // The staking pallet seems to only support 64 bit balances
                    let staking_balance: T::CurrencyBalance = T::CurrencyBalance::from(
                        UniqueSaturatedInto::<u64>::unique_saturated_into(request.amount),
                    );
                    pallet_staking::Pallet::<T>::bond(
                        staker_signer.clone().into(),
                        staking_balance,
                        pallet_staking::RewardDestination::Staked,
                    )?;
                    pallet_staking::Pallet::<T>::nominate(
                        staker_signer.clone().into(),
                        request.nominations,
                    );
                }
            }
            (_, _, _) => {
                //TODO
            }
        }
        Ok(())
    }

    pub struct NominateRequest<T: frame_system::Config> {
        pub nominator_id: T::AccountId,
        pub nominations: Vec<<T::Lookup as StaticLookup>::Source>,
    }

    pub fn parse_nominate<T>(oc_table: OnChainTable) -> DispatchResult
    where
        T: Config,
    {
        // List of staker ids
        let nominators = oc_table.get_varchars_by_column("NOMINATOR");
        // list of target nodes as a comma separated string (?)
        let target_nodes = oc_table.get_varchars_by_column("NODES");

        match (nominators, target_nodes) {
            (Some(nominators), Some(nominations)) => {
                // Build a set of NominateRequest objects
                let nominate_requests: Vec<NominateRequest<T>> = nominators
                    .iter()
                    .enumerate()
                    .filter_map(|(i, nominator)| {
                        Some(NominateRequest {
                            nominator_id: eth_address_to_substrate_account_id::<T>(nominator)
                                .ok()?,
                            nominations: nominations[i]
                                .split(',')
                                .filter_map(|s| {
                                    Some(<T as frame_system::Config>::Lookup::unlookup(
                                        eth_address_to_substrate_account_id::<T>(s.trim()).ok()?,
                                    ))
                                })
                                .collect(),
                        })
                    })
                    .collect();

                for request in nominate_requests {
                    let nominator_signer: OriginFor<T> =
                        RawOrigin::Signed(request.nominator_id).into();
                    pallet_staking::Pallet::<T>::nominate(
                        nominator_signer,
                        request.nominations.clone(),
                    )?;
                }
            }
            (_, _) => {
                //TODO
            }
        }

        Ok(())
    }

    pub struct UnstakeInitiatedRequest<T: frame_system::Config> {
        pub staker_id: T::AccountId,
    }

    pub fn parse_unstakeinitiated<T>(oc_table: OnChainTable) -> DispatchResult
    where
        T: Config,
    {
        match oc_table.get_varchars_by_column("STAKER") {
            Some(stakers) => {
                let unstake_requests: Vec<UnstakeInitiatedRequest<T>> = stakers
                    .iter()
                    .filter_map(|staker_id| {
                        Some(UnstakeInitiatedRequest {
                            staker_id: eth_address_to_substrate_account_id::<T>(staker_id).ok()?,
                        })
                    })
                    .collect();

                for request in unstake_requests {
                    let staker_signer: OriginFor<T> =
                        RawOrigin::Signed(request.staker_id.clone()).into();

                    let raw_balance: u128 =
                        pallet_balances::Pallet::<T>::free_balance(request.staker_id)
                            .unique_saturated_into();
                    let staking_balance: T::CurrencyBalance = T::CurrencyBalance::from(
                        UniqueSaturatedInto::<u64>::unique_saturated_into(raw_balance),
                    );
                    pallet_staking::Pallet::<T>::unbond(staker_signer, staking_balance);
                }
            }
            None => {
                //TODO
            }
        }
        Ok(())
    }

    pub struct NodeRegisteredRequest<T: frame_system::Config> {
        pub node_public_key: String,
        pub operator_id: T::AccountId,
    }

    pub fn parse_noderegistered<T>(oc_table: OnChainTable) -> DispatchResult
    where
        T: Config,
    {
        let node_pks = oc_table.get_varchars_by_column("NODEPUBLICKEY");
        let operators = oc_table.get_varchars_by_column("OPERATOR");

        match (node_pks, operators) {
            (Some(node_keys), Some(operator_ids)) => {
                // Build a set of NodeRegisteredRequest objects
                let register_requests: Vec<NodeRegisteredRequest<T>> = operator_ids
                    .iter()
                    .enumerate()
                    .filter_map(|(i, id)| {
                        Some(NodeRegisteredRequest {
                            operator_id: eth_address_to_substrate_account_id::<T>(id).ok()?,
                            node_public_key: node_keys[i].clone(),
                        })
                    })
                    .collect();

                for request in register_requests {
                    let operator_signer: OriginFor<T> =
                        RawOrigin::Signed(request.operator_id).into();
                    let prefs = ValidatorPrefs {
                        commission: sp_runtime::Perbill::zero(),
                        blocked: false,
                    };

                    pallet_staking::Pallet::<T>::validate(operator_signer, prefs);
                }
            }
            (_, _) => {
                //TODO
            }
        }
        Ok(())
    }

    pub struct UnstakeCancelledRequest<T: frame_system::Config> {
        pub staker_id: T::AccountId,
    }

    /// This function might call `pallet_staking::Pallet::rebond(...)`
    /// or some custom logic to cancel an unbond in progress.
    pub fn parse_unstakecancelled<T>(oc_table: OnChainTable) -> DispatchResult
    where
        T: Config,
    {
        let stakers = oc_table.get_varchars_by_column("STAKER");

        match stakers {
            Some(stakers) => {
                // Build a set of UnstakeCancelledRequest objects
                let requests: Vec<UnstakeCancelledRequest<T>> = stakers
                    .iter()
                    .filter_map(|staker_id| {
                        Some(UnstakeCancelledRequest {
                            staker_id: eth_address_to_substrate_account_id::<T>(staker_id).ok()?,
                        })
                    })
                    .collect();

                for request in requests {
                    let staker_signer = RawOrigin::Signed(&request.staker_id);

                    // TODO get the rebond amount
                    // pallet_staking::Pallet::<T>::rebond(&staker_signer, amount_to_rebond)?;
                }
            }
            None => {
                //TODO
            }
        }

        Ok(())
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
