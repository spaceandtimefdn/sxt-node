use codec::Encode;
use proof_of_sql_commitment_map::CommitmentScheme;
use snafu::{ResultExt, Snafu};
use sp_core::crypto::AccountId32;
use sp_core::{ByteArray, Decode};
use subxt::{OnlineClient, PolkadotConfig};
use sxt_core::sxt_chain_runtime;
use sxt_core::tables::TableIdentifier;

/// Errors that may occur while fetching blockchain data.
#[derive(Debug, Snafu)]
pub enum FetchError {
    /// Error encountered while iterating over account storage in the blockchain.
    ///
    /// This occurs when accessing and processing account balances stored on-chain.
    #[snafu(display("Failed to iterate over accounts: {source}"))]
    AccountStorageIteration {
        /// The underlying error from Substrate's storage iteration.
        source: subxt::Error,
    },

    /// Error encountered while iterating over commitment storage in the blockchain.
    ///
    /// This happens when accessing and processing commitment records stored on-chain.
    #[snafu(display("Failed to iterate over commitments: {source}"))]
    CommitmentStorageIteration {
        /// The underlying error from Substrate's storage iteration.
        source: subxt::Error,
    },

    /// Error when extracting an account ID due to an invalid length.
    ///
    /// This typically means that the extracted key is too short or improperly formatted.
    #[snafu(display(
        "Error extracting account ID, invalid length, key: {}",
        hex::encode(key)
    ))]
    InvalidAccountId {
        /// The raw key that failed extraction.
        key: Vec<u8>,
    },

    /// Error when decoding an account ID from storage.
    ///
    /// This happens when the key format does not match expected encoding.
    #[snafu(display("Failed to decode account id: {} error : {source}", hex::encode(key)))]
    DecodeAccountId {
        /// The underlying decoding error from `codec`.
        source: codec::Error,
        /// The raw key that failed decoding.
        key: Vec<u8>,
    },

    /// Error when key bytes are too short to extract the required data.
    ///
    /// This usually indicates corrupted or incomplete key storage.
    #[snafu(display("Key bytes too short: {}", hex::encode(key)))]
    KeyBytesTooShort {
        /// The raw key that was too short.
        key: Vec<u8>,
    },

    /// Error when decoding a `TableIdentifier` but not enough bytes remain for `CommitmentScheme`.
    ///
    /// This suggests that the storage key is malformed or incomplete.
    #[snafu(display(
        "Not enough bytes left for CommitmentScheme after decoding TableIdentifier: {:?}",
        key
    ))]
    NotEnoughBytesForCommitment {
        /// The raw key that failed decoding.
        key: Vec<u8>,
    },

    /// Error when decoding a `TableIdentifier` from storage.
    ///
    /// Occurs if the identifier format does not match expected encoding.
    #[snafu(display("Failed to decode TableIdentifier: {source}"))]
    DecodeTableIdentifier {
        /// The underlying decoding error from `codec`.
        source: codec::Error,
    },

    /// Error when decoding a `CommitmentScheme` from storage.
    ///
    /// This happens when commitment scheme bytes do not match expected encoding.
    #[snafu(display("Failed to decode CommitmentScheme: {source}"))]
    DecodeCommitmentScheme {
        /// The underlying decoding error from `codec`.
        source: codec::Error,
    },
}

/// Fetches account balances from the blockchain.
///
/// # Arguments
/// * `api` - Reference to the Substrate API client.
/// * `block_hash` - The hash of the block to fetch data from.
///
/// # Returns
/// A `Result` containing a vector of encoded account balances, or an error.
pub async fn fetch_accounts_data(
    api: &OnlineClient<PolkadotConfig>,
    block_hash: subxt::utils::H256,
) -> Result<Vec<String>, FetchError> {
    let mut data = Vec::new();

    let account_iter = sxt_chain_runtime::api::storage().system().account_iter();

    let mut accounts = api
        .storage()
        .at(block_hash)
        .iter(account_iter)
        .await
        .context(AccountStorageIterationSnafu)?;

    while let Some(result) = accounts.next().await {
        match result {
            Ok(kv) => match extract_account_id_from_balance_key(&kv.key_bytes) {
                Ok(account_id) => {
                    let free_balance = kv.value.data.free;
                    data.push(encode_account_leaf(account_id, free_balance));
                }
                Err(err) => {
                    log::error!("Error extracting account ID: {:?}", err);
                }
            },
            Err(err) => {
                log::error!("Error iterating over accounts: {:?}", err);
            }
        }
    }

    Ok(data)
}

/// Extracts an `AccountId32` from a given balance key.
///
/// # Arguments
/// * `key_bytes` - A slice of bytes representing the key.
///
/// # Returns
/// A `Result` containing the extracted `AccountId32`, or an error.
fn extract_account_id_from_balance_key(key_bytes: &[u8]) -> Result<AccountId32, FetchError> {
    if key_bytes.len() < 48 {
        return Err(FetchError::InvalidAccountId {
            key: key_bytes.to_vec(),
        });
    }

    let account_id_bytes = &key_bytes[48..];

    AccountId32::decode(&mut &account_id_bytes[..]).context(DecodeAccountIdSnafu {
        key: key_bytes.to_vec(),
    })
}

/// Encodes an account leaf node for inclusion in a Merkle tree.
///
/// # Arguments
/// * `account_id` - The `AccountId32` of the account.
/// * `free_balance` - The free balance associated with the account.
///
/// # Returns
/// A `String` containing the hex-encoded representation of the account leaf.
pub fn encode_account_leaf(account_id: AccountId32, free_balance: u128) -> String {
    let mut encoded = account_id.as_slice().to_vec();
    encoded.extend_from_slice(&free_balance.to_be_bytes());
    hex::encode(encoded)
}

/// Fetches commitment data from the blockchain.
///
/// # Arguments
/// * `api` - Reference to the Substrate API client.
/// * `block_hash` - The hash of the block to fetch commitments from.
///
/// # Returns
/// A `Result` containing a vector of encoded commitments, or an error.
pub async fn fetch_commitments(
    api: &OnlineClient<PolkadotConfig>,
    block_hash: subxt::utils::H256,
) -> Result<Vec<String>, FetchError> {
    let mut data = Vec::new();

    let commitment_addr = sxt_chain_runtime::api::storage()
        .commitments()
        .commitment_storage_map_iter();

    let mut commitments = api
        .storage()
        .at(block_hash)
        .iter(commitment_addr)
        .await
        .context(CommitmentStorageIterationSnafu)?;

    while let Some(Ok(kv)) = commitments.next().await {
        match decode_key_bytes(&kv.key_bytes) {
            Err(e) => {
                log::error!("Error extracting table identifier and scheme: {:?}", e);
            }
            Ok((ident, scheme)) => {
                let commitment = kv.value.data.0;

                let mut encoded = ident.name.encode();
                encoded.extend_from_slice(&ident.namespace.encode());
                encoded.extend_from_slice(&scheme.encode());
                encoded.extend_from_slice(&commitment);

                data.push(hex::encode(encoded));
            }
        }
    }

    Ok(data)
}

/// Decodes key bytes into a `TableIdentifier` and a `CommitmentScheme`.
///
/// # Arguments
/// * `key_bytes` - A slice of bytes representing the key.
///
/// # Returns
/// A `Result` containing the decoded `(TableIdentifier, CommitmentScheme)`, or an error.
pub fn decode_key_bytes(
    mut key_bytes: &[u8],
) -> Result<(TableIdentifier, CommitmentScheme), FetchError> {
    if key_bytes.len() < 48 {
        return Err(FetchError::KeyBytesTooShort {
            key: key_bytes.to_vec(),
        });
    }
    key_bytes = &key_bytes[48..];

    let table_identifier =
        TableIdentifier::decode(&mut key_bytes).context(DecodeTableIdentifierSnafu)?;

    if key_bytes.len() < 16 {
        return Err(FetchError::NotEnoughBytesForCommitment {
            key: key_bytes.to_vec(),
        });
    }
    key_bytes = &key_bytes[16..];

    let commitment_scheme =
        CommitmentScheme::decode(&mut key_bytes).context(DecodeCommitmentSchemeSnafu)?;

    Ok((table_identifier, commitment_scheme))
}

/// Fetches both commitments and account data for a given block.
///
/// # Arguments
/// * `api` - Reference to the Substrate API client.
/// * `block_hash` - The hash of the block to fetch data from.
///
/// # Returns
/// A `Result` containing a tuple of encoded commitments and account balances, or an error.
pub async fn commitments_and_accounts(
    api: &OnlineClient<PolkadotConfig>,
    block_hash: subxt::utils::H256,
) -> Result<(Vec<String>, Vec<String>), FetchError> {
    let commitment_data = fetch_commitments(api, block_hash).await?;
    let account_data = fetch_accounts_data(api, block_hash).await?;

    Ok((commitment_data, account_data))
}
