use std::marker::PhantomData;

use frame_support::{Blake2_128Concat, WeakBoundedVec};
use pallet_balances::BalanceLock;
use sxt_core::system_contracts::ContractInfo;

use crate::PrefixFoliate;

/// The lock id used for staking balance locks.
pub const STAKING_BALANCE_LOCK_ID: &[u8; 8] = b"staking ";

/// [`PrefixFoliate`] for the `Locks` storage in `pallet_balances`, filtered for the staking locks.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LocksStakingPrefixFoliate<T: pallet_balances::Config<()>>(PhantomData<T>);

impl<T> PrefixFoliate for LocksStakingPrefixFoliate<T>
where
    T: pallet_balances::Config<(), Balance = u128>,
{
    type StorageInstance = pallet_balances::_GeneratedPrefixForStorageLocks<T, ()>;
    type HashAndKeyTuple = ((Blake2_128Concat, T::AccountId),);
    type Value = (
        WeakBoundedVec<BalanceLock<T::Balance>, T::MaxLocks>,
        ContractInfo,
    );

    // only encode the amount for staking locks (bigendian, 248-bit), otherwise 0
    fn leaf_encode_value((locks, ContractInfo { chain_id, address }): Self::Value) -> Vec<u8> {
        let chain_id_bytes = {
            let mut bytes = [0u8; 32];
            chain_id.to_big_endian(&mut bytes);
            bytes
        };

        std::iter::repeat(0)
            // first we pad with 15 0-bytes
            .take(15)
            // then we add the 16 bytes from the on-chain u128
            .chain(
                locks
                    .into_iter()
                    .find(|balance_lock| &balance_lock.id == STAKING_BALANCE_LOCK_ID)
                    .map(|balance_lock| balance_lock.amount)
                    .unwrap_or(0)
                    .to_be_bytes(),
            )
            // total we have 31 bytes, aka 248 bits
            .chain(chain_id_bytes)
            .chain(address.to_fixed_bytes())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use pallet_balances::Reasons;
    use sp_core::crypto::AccountId32;
    use sp_core::{H160, U256};
    use sxt_runtime::Runtime;

    use super::*;

    #[test]
    fn we_can_leaf_encode_account_id() {
        let raw_bytes: [u8; 32] = (0u8..32).collect::<Vec<_>>().try_into().unwrap();

        let account_id = AccountId32::new(raw_bytes);

        let actual = LocksStakingPrefixFoliate::<Runtime>::leaf_encode_key((account_id,));

        assert_eq!(actual, raw_bytes);
    }

    #[test]
    fn we_can_leaf_encode_staking_balance_lock() {
        let staking_lock = BalanceLock::<u128> {
            amount: 257,
            id: *STAKING_BALANCE_LOCK_ID,
            reasons: Reasons::All,
        };
        let misc_lock = BalanceLock::<u128> {
            amount: 515,
            id: *b"otherloc",
            reasons: Reasons::All,
        };

        let locks = vec![misc_lock, staking_lock].try_into().unwrap();

        let chain_id = U256::from(1028u32);
        let address = H160::from_str("0x000102030405060708090a0b0c0d0e0f10111213").unwrap();

        let contract_info = ContractInfo { chain_id, address };

        let actual =
            LocksStakingPrefixFoliate::<Runtime>::leaf_encode_value((locks, contract_info));

        let expected_amount = std::iter::repeat(0).take(29).chain([1, 1]);

        let expected_chain_id = std::iter::repeat(0).take(30).chain([4, 4]);

        let expected_address = 0u8..20;

        let expected = expected_amount
            .chain(expected_chain_id)
            .chain(expected_address)
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn we_can_leaf_encode_balance_lock_without_staking() {
        let misc_lock = BalanceLock::<u128> {
            amount: 515,
            id: *b"otherloc",
            reasons: Reasons::All,
        };

        let locks = vec![misc_lock].try_into().unwrap();

        let chain_id = U256::from(1028u32);
        let address = H160::from_str("0x000102030405060708090a0b0c0d0e0f10111213").unwrap();

        let contract_info = ContractInfo { chain_id, address };

        let actual =
            LocksStakingPrefixFoliate::<Runtime>::leaf_encode_value((locks, contract_info));

        let expected_amount = [0u8; 31];

        let expected_chain_id = std::iter::repeat(0).take(30).chain([4, 4]);

        let expected_address = 0u8..20;

        let expected = expected_amount
            .into_iter()
            .chain(expected_chain_id)
            .chain(expected_address)
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}
