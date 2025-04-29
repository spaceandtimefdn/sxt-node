use std::collections::HashMap;

use axum::http::StatusCode;
use axum::Json;
use hex::FromHex;
use subxt::utils::H256;
use subxt::Metadata;
use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::smartcontracts::{
    Contract,
    ContractDetails,
    EventDetails,
    NormalContract,
    ProxyContract,
};
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::{
    IndexerMode as RuntimeMode,
    Source as RuntimeSource,
};
use sxt_core::sxt_chain_runtime::api::DispatchError;

use crate::model::{ApiContract, ApiResponse, EventMapping};

/// Parses a hexadecimal string into an `H256` hash.
///
/// This function trims a leading `0x` if present and then attempts to decode the hex string into
/// a 32-byte array, which is then converted into an `H256` type.
///
/// # Arguments
/// * `hex_str` - A string slice containing the hexadecimal representation of the hash.
///
/// # Returns
/// * `Ok(H256)` - If the hex string is valid and successfully parsed.
/// * `Err(String)` - If the hex string is invalid or improperly formatted.
pub fn parse_h256_from_hex(hex_str: &str) -> Result<H256, String> {
    let hex_str = hex_str.trim_start_matches("0x"); // Remove "0x" prefix if present

    // Decode hex string into bytes
    let bytes = <[u8; 32]>::from_hex(hex_str).map_err(|_| "Invalid hex string")?;

    // Convert to H256
    Ok(H256::from(bytes))
}

/// Constructs a `400 Bad Request` API response.
///
/// # Arguments
/// * `message` - A message describing the reason for the bad request.
///
/// # Returns
/// A tuple containing `StatusCode::BAD_REQUEST` and a JSON `ApiResponse` payload.
pub fn bad_request(message: &str) -> (StatusCode, Json<ApiResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiResponse {
            success: false,
            err_msg: Some(message.to_string()),
            tx_hash: None,
        }),
    )
}

/// Constructs a `500 Internal Server Error` API response.
///
/// # Arguments
/// * `message` - A message describing the internal server error.
///
/// # Returns
/// A tuple containing `StatusCode::INTERNAL_SERVER_ERROR` and a JSON `ApiResponse` payload.
pub fn internal_server_error(message: &str) -> (StatusCode, Json<ApiResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse {
            success: false,
            err_msg: Some(message.to_string()),
            tx_hash: None,
        }),
    )
}

/// Constructs a `404 Not Found` API response.
///
/// # Arguments
/// * `message` - A message describing the missing resource.
///
/// # Returns
/// A tuple containing `StatusCode::NOT_FOUND` and a JSON `ApiResponse` payload.
pub fn not_found(message: &str) -> (StatusCode, Json<ApiResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse {
            success: false,
            err_msg: Some(message.to_string()),
            tx_hash: None,
        }),
    )
}

/// Decodes a `ModuleError` from Substrate into a human-readable format.
///
/// This function extracts the error name and description using the blockchain's metadata,
/// allowing developers to better understand and debug transaction failures.
///
/// # Arguments
/// * `dispatch_error` - A reference to the `DispatchError` containing the error details.
/// * `metadata` - A reference to the blockchain's metadata for error lookup.
///
/// # Returns
/// * `Some(String)` - A formatted error message if the error is found in metadata.
/// * `None` - If the error cannot be resolved using the metadata.
pub fn decode_system_module_error(
    dispatch_error: &DispatchError,
    metadata: &Metadata,
) -> Option<String> {
    if let DispatchError::Module(module_error) = dispatch_error {
        if let Some(pallet) = metadata.pallet_by_index(module_error.index) {
            if let Some(error_details) = pallet.error_variant_by_index(module_error.error[0]) {
                let error_name = &error_details.name;
                let error_doc = error_details.docs.join(" "); // Some errors have descriptions

                return Some(format!(
                    "Pallet: {}, Error: {} ({})",
                    "System", error_name, error_doc
                ));
            }
        }
    }
    None
}

/// Converts `Option<BoundedVec<EventDetails>>` into `Vec<EventMapping>`.
///
/// This function transforms blockchain event details into a more accessible format
/// used by the API response structure.
///
/// # Arguments
/// * `event_details` - An optional bounded vector of event details.
///
/// # Returns
/// * `Vec<EventMapping>` - A list of API-compatible event mappings.
pub fn convert_event_details_from_chain(
    event_details: Option<BoundedVec<EventDetails>>,
) -> Vec<EventMapping> {
    match event_details {
        Some(bound_vec) => bound_vec
            .0
            .into_iter()
            .map(|event| EventMapping {
                name: String::from_utf8_lossy(&event.name.0).into_owned(),
                signature: String::from_utf8_lossy(&event.signature.0).into_owned(),
                table: String::from_utf8_lossy(&event.table.0).into_owned(),
            })
            .collect(),
        None => Vec::new(), // Return empty vector if no events exist
    }
}

/// Converts `Vec<EventMapping>` into `Option<BoundedVec<EventDetails>>`.
///
/// This function transforms API event mappings into a format suitable for blockchain storage.
///
/// # Arguments
/// * `events` - A vector of event mappings from the API request.
///
/// # Returns
/// * `Option<BoundedVec<EventDetails>>` - A bounded vector containing event details, or `None` if empty.
pub fn convert_event_details_to_chain(
    events: Vec<EventMapping>,
) -> Option<BoundedVec<EventDetails>> {
    if events.is_empty() {
        return None; // Return None if there are no events
    }

    Some(BoundedVec(
        events
            .into_iter()
            .map(|event| EventDetails {
                name: BoundedVec(event.name.into_bytes()),
                signature: BoundedVec(event.signature.into_bytes()),
                table: BoundedVec(event.table.into_bytes()),
            })
            .collect(),
    ))
}

/// Extracts a required parameter from the request query map.
///
/// # Arguments
/// * `params` - A reference to a `HashMap<String, String>` containing the query parameters.
/// * `key` - The name of the parameter to extract.
///
/// # Returns
/// * `Ok(String)` - The extracted value if found.
/// * `Err((StatusCode, Json<ApiResponse>))` - A `400 Bad Request` response if the parameter is missing.
pub fn extract_param(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<String, (StatusCode, Json<ApiResponse>)> {
    params
        .get(key)
        .cloned()
        .ok_or_else(|| bad_request(&format!("Missing required query parameter: {}", key)))
}

/// Maps a `Contract` from storage into an API-compatible `ApiContract` response.
///
/// # Arguments
/// * `contract` - The contract data stored on the blockchain.
/// * `source` - The source blockchain where the contract is deployed.
/// * `address` - The contract's address in a bounded vector format.
///
/// # Returns
/// * `ApiContract` - A representation of the contract suitable for API responses.
pub fn map_contract_to_api(
    contract: Contract,
    source: &RuntimeSource,
    address: BoundedVec<u8>,
) -> ApiContract {
    match contract {
        Contract::Normal(NormalContract {
            details:
                ContractDetails {
                    abi,
                    starting_block,
                    target_schema,
                    contract_name,
                    event_details,
                    ..
                },
        }) => ApiContract::Normal {
            source: source_to_string(source),
            address: bytes_to_string(&address.0),
            abi: bytes_option_to_string(abi),
            starting_block,
            target_schema: bytes_option_to_string(target_schema),
            contract_name: bytes_option_to_string(contract_name),
            events: convert_event_details_from_chain(event_details),
            ddl_statement: Some("".into()),
        },

        Contract::Proxy(ProxyContract {
            details:
                ContractDetails {
                    starting_block,
                    contract_name,
                    event_details,
                    ..
                },
            implementation,
        }) => ApiContract::Proxy {
            source: source_to_string(source),
            address: bytes_to_string(&address.0),
            implementation_address: bytes_to_string(&implementation.details.address.0),
            abi: bytes_option_to_string(implementation.details.abi),
            starting_block,
            target_schema: bytes_option_to_string(implementation.details.target_schema),
            contract_name: bytes_option_to_string(contract_name),
            events: convert_event_details_from_chain(event_details),
            ddl_statement: Some("".into()),
        },
    }
}

/// Converts a `BoundedVec<u8>` into a `String`, handling UTF-8 conversion.
pub fn bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Converts an `Option<BoundedVec<u8>>` into an `Option<String>`, handling UTF-8 conversion.
pub fn bytes_option_to_string(option: Option<BoundedVec<u8>>) -> Option<String> {
    option.map(|bv| bytes_to_string(&bv.0))
}

/// Converts a string into the corresponding `RuntimeMode` variant.
///
/// This function normalizes the input string by converting it to lowercase
/// and then maps it to one of the predefined `RuntimeMode` variants.
/// If the input string contains a custom mode (`SmartContract` or `UserCreated`),
/// the function extracts and encodes the inner value.
///
/// # Arguments
/// * `mode` - A string slice representing the mode.
///
/// # Returns
/// * `RuntimeMode` - The corresponding enum variant.
///
/// ```
pub fn string_to_mode(mode: &str) -> RuntimeMode {
    match mode.to_lowercase().as_str() {
        "core" => RuntimeMode::Core,
        "full" => RuntimeMode::Full,
        "pricefeeds" | "price_feeds" => RuntimeMode::PriceFeeds,
        mode if mode.starts_with("smartcontract:") => {
            let inner_value = mode.trim_start_matches("smartcontract:");
            RuntimeMode::SmartContract(BoundedVec(inner_value.as_bytes().to_vec()))
        }
        mode if mode.starts_with("usercreated:") => {
            let inner_value = mode.trim_start_matches("usercreated:");
            RuntimeMode::UserCreated(BoundedVec(inner_value.as_bytes().to_vec()))
        }
        _ => RuntimeMode::UserCreated(BoundedVec(mode.as_bytes().to_vec())),
    }
}

/// Converts a string into the corresponding `RuntimeSource` variant.
/// If the input string does not match a known source, it is treated as `UserCreated`.
pub fn string_to_source(source: &str) -> RuntimeSource {
    match source.to_lowercase().as_str() {
        "ethereum" => RuntimeSource::Ethereum,
        "sepolia" => RuntimeSource::Sepolia,
        "bitcoin" => RuntimeSource::Bitcoin,
        "polygon" => RuntimeSource::Polygon,
        "zksyncera" => RuntimeSource::ZkSyncEra,
        other => RuntimeSource::UserCreated(BoundedVec(other.into())),
    }
}

/// Converts a `RuntimeSource` into a human-readable string.
/// For `UserCreated`, it extracts the stored value as a UTF-8 string.
pub fn source_to_string(source: &RuntimeSource) -> String {
    match source {
        RuntimeSource::Ethereum => "Ethereum".to_owned(),
        RuntimeSource::Sepolia => "Sepolia".to_owned(),
        RuntimeSource::Bitcoin => "Bitcoin".to_owned(),
        RuntimeSource::Polygon => "Polygon".to_owned(),
        RuntimeSource::ZkSyncEra => "ZkSyncEra".to_owned(),
        RuntimeSource::UserCreated(bounded_vec) => {
            // Convert the bounded vector into a string safely
            match String::from_utf8(bounded_vec.0.clone()) {
                Ok(custom_source) => custom_source,
                Err(_) => "InvalidUserCreatedSource".to_owned(), // Handle possible UTF-8 errors gracefully
            }
        }
    }
}
