use std::marker::PhantomData;

use frame_support::Blake2_128Concat;
use frame_system::AccountInfo;

use crate::PrefixFoliate;

/// [`PrefixFoliate`] for the `Account` storage in `frame_system`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountPrefixFoliate<T: frame_system::Config>(PhantomData<T>);

impl<T> PrefixFoliate for AccountPrefixFoliate<T>
where
    T: frame_system::Config<AccountData = pallet_balances::AccountData<u128>>,
{
    type StorageInstance = frame_system::_GeneratedPrefixForStorageAccount<T>;
    type HashAndKeyTuple = ((Blake2_128Concat, T::AccountId),);
    type Value = AccountInfo<T::Nonce, T::AccountData>;

    // only encode the free balance (bigendian)
    fn leaf_encode_value(value: Self::Value) -> Vec<u8> {
        value.data.free.to_be_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use pallet_balances::{AccountData, ExtraFlags};
    use sp_core::crypto::AccountId32;
    use sxt_runtime::Runtime;

    use super::*;

    #[test]
    fn we_can_leaf_encode_account_id() {
        let raw_bytes: [u8; 32] = (0u8..32).collect::<Vec<_>>().try_into().unwrap();

        let account_id = AccountId32::new(raw_bytes);

        let actual = AccountPrefixFoliate::<Runtime>::leaf_encode_key((account_id,));

        assert_eq!(actual, raw_bytes);
    }

    #[test]
    fn we_can_leaf_encode_account_balance() {
        let data = AccountData::<u128> {
            free: 257,
            reserved: 1024,
            frozen: 514,
            flags: ExtraFlags::default(),
        };

        let account_info = AccountInfo {
            data,
            ..Default::default()
        };

        let actual = AccountPrefixFoliate::<Runtime>::leaf_encode_value(account_info);

        let expected = std::iter::repeat(0)
            .take(14)
            .chain([1, 1])
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}
