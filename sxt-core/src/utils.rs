use alloc::string::String;
use alloc::vec::Vec;

use codec::Decode;
use hex::FromHex;
use sp_core::crypto::AccountId32;
use sp_runtime::traits::StaticLookup;
use sp_runtime::DispatchError;

/// Convert the given comma separated list of ethereum public addresses to a Vec of
/// substrate compatible Account Lookups
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

/// Convert the supplied AccountId32 to the runtime's AccountId type
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
