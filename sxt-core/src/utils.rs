use alloc::vec::Vec;

use codec::Decode;
use frame_system::Config as SystemConfig;
use hex::FromHex;
use sp_core::crypto::AccountId32;
use sp_runtime::traits::StaticLookup;
use sp_runtime::DispatchError;

type AddressListParseResult<T> = Result<
    Vec<Result<<<T as SystemConfig>::Lookup as StaticLookup>::Source, DispatchError>>,
    serde_json::Error,
>;

/// Parse an address list from the custom garfield encoding
pub fn parse_address_list_json<T: frame_system::Config>(input: &str) -> AddressListParseResult<T> {
    let raw_addresses: Vec<&str> = serde_json::from_str(input)?;

    let results = raw_addresses
        .into_iter()
        .map(|s| account_id_from_str::<T>(s).map(T::Lookup::unlookup))
        .collect();

    Ok(results)
}

/// Build a substrate account id from a hex encoded string
pub fn account_id_from_str<T: frame_system::Config>(
    addr: &str,
) -> Result<T::AccountId, DispatchError> {
    let hex_str = addr.trim_start_matches("0x");
    let raw_bytes = hex::decode(hex_str).map_err(|_| "Invalid hex address")?;
    let raw_bytes: [u8; 32] = raw_bytes
        .try_into()
        .map_err(|_| "Could not coerce account into 32 bytes")?;

    convert_account_id::<T>(sp_runtime::AccountId32::from(raw_bytes))
}

/// This function takes a Ethereum Wallet Address and transforms it into a Substrate
/// compatible AccountId
pub fn eth_address_to_substrate_account_id<T: frame_system::Config>(
    eth_addr_hex: &str,
) -> Result<T::AccountId, DispatchError> {
    // Strip optional "0x" prefix, decode the remaining hex.
    let hex_str = eth_addr_hex.trim_start_matches("0x");
    let raw_bytes = hex::decode(hex_str).map_err(|_| "Invalid hex address")?;

    let raw_addr: [u8; 20] = raw_bytes
        .try_into()
        .map_err(|_| "Expected 20-byte Ethereum address")?;

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

/// Common bincode configuration for encoding/decoding of proof-of-sql objects.
pub fn proof_of_sql_bincode_config<const ALLOCATION_LIMIT: usize>() -> impl bincode::config::Config
{
    bincode::config::legacy()
        .with_fixed_int_encoding()
        .with_big_endian()
        .with_limit::<ALLOCATION_LIMIT>()
}
