use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use on_chain_table::{OnChainTable, OnChainColumn};
use anyhow::{anyhow, Error};
use arrow::array::{Array, BooleanArray, Decimal128Array, Decimal256Array, Int16Array, Int32Array, Int64Array, Int8Array, LargeStringArray, RecordBatch, StringArray, TimestampMicrosecondArray};
use log::{error, info};
use sqlparser::ast::{Ident, Statement};
use sqlparser::dialect::GenericDialect;
use std::io::Cursor;
use proof_of_sql::base::math::decimal::Precision;
use sqlparser::parser::Parser as SqlParser;
use subxt::backend::rpc::reconnecting_rpc_client::RpcClient;
use subxt::config::polkadot::PolkadotExtrinsicParamsBuilder as Params;
use subxt::utils::AccountId32;
use arrow::ipc::writer::StreamWriter;
use proof_of_sql::base::posql_time::PoSQLTimeUnit;
use subxt::{OnlineClient, PolkadotConfig};
use subxt::ext::sp_core::U256;
use subxt_signer::sr25519::Keypair;
use subxt_signer::SecretUri;
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_core::sxt_chain_runtime::api::runtime_types::pallet_tables::pallet::{CommitmentCreationCmd, UpdateTable};
use sxt_core::sxt_chain_runtime::api::runtime_types::proof_of_sql_commitment_map::commitment_scheme::CommitmentSchemeFlags;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::{
    IndexerMode,
    InsertQuorumSize,
    Source,
    SourceAndMode,
    TableIdentifier, TableType,
};
use sxt_core::sxt_chain_runtime::api::tx;
use tokio::sync::Mutex;
use url::Url;
use sxt_core::sxt_chain_runtime::api::indexing::calls::types::submit_data::{BatchId, Data, Table};
use crate::common;

fn get_staking_message(eth_wallet: &str, amount: U256) -> Data {
    let expected = OnChainTable::try_from_iter([
        (Ident::new("block_number"), OnChainColumn::BigInt(vec![1])),
        (
            Ident::new("time_stamp"),
            OnChainColumn::TimestampTZ(PoSQLTimeUnit::Microsecond, None, vec![1]),
        ),
        (
            Ident::new("transaction_hash"),
            OnChainColumn::VarChar(vec!["asdf".to_string()]),
        ),
        (Ident::new("event_index"), OnChainColumn::Int(vec![1])),
        (
            Ident::new("contract_address"),
            OnChainColumn::VarChar(vec!["asdf".to_string()]),
        ),
        (
            Ident::new("decode_error"),
            OnChainColumn::VarChar(vec!["asdf".to_string()]),
        ),
        (
            Ident::new("staker"),
            OnChainColumn::VarChar(vec![eth_wallet.to_string()]),
        ),
        (
            Ident::new("amount"),
            OnChainColumn::Decimal75(Precision::new(75).unwrap(), 0, vec![amount]),
        ),
    ])
    .unwrap();

    let record_batch = RecordBatch::from(expected);

    // Convert the record batch to Arrow's IPC format so we can submit it
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = StreamWriter::try_new(&mut buffer, &record_batch.schema()).unwrap();
        writer.write(&record_batch).unwrap();
        writer.finish().unwrap();
    }
    BoundedVec(buffer.into_inner())
}

fn get_session_keys_message(eth_wallet: &str, session_keys: &str) -> Data {
    let expected = OnChainTable::try_from_iter([
        (Ident::new("block_number"), OnChainColumn::BigInt(vec![1])),
        (
            Ident::new("time_stamp"),
            OnChainColumn::TimestampTZ(PoSQLTimeUnit::Microsecond, None, vec![1]),
        ),
        (
            Ident::new("transaction_hash"),
            OnChainColumn::VarChar(vec!["asdf".to_string()]),
        ),
        (Ident::new("event_index"), OnChainColumn::Int(vec![1])),
        (
            Ident::new("contract_address"),
            OnChainColumn::VarChar(vec!["asdf".to_string()]),
        ),
        (
            Ident::new("decode_error"),
            OnChainColumn::VarChar(vec!["asdf".to_string()]),
        ),
        (
            Ident::new("sender"),
            OnChainColumn::VarChar(vec![eth_wallet.to_string()]),
        ),
        (
            Ident::new("body"),
            OnChainColumn::VarChar(vec![session_keys.to_string()]),
        ),
        (
            Ident::new("nonce"),
            OnChainColumn::Decimal75(Precision::new(75).unwrap(), 0, vec![1.into()]),
        ),
    ])
    .unwrap();

    let record_batch = RecordBatch::from(expected);

    // Convert the record batch to Arrow's IPC format so we can submit it
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = StreamWriter::try_new(&mut buffer, &record_batch.schema()).unwrap();
        writer.write(&record_batch).unwrap();
        writer.finish().unwrap();
    }
    BoundedVec(buffer.into_inner())
}

/// Send to substrate
async fn send_staking_message(
    eth_wallet: &str,
    ammount: U256,
    client: &Arc<Mutex<OnlineClient<PolkadotConfig>>>,
    keypair: &Keypair,
    nonce: Arc<AtomicU64>,
) {
    let client = client.lock().await;

    let staking_table = Table::from(TableIdentifier {
        name: BoundedVec("STAKED".as_bytes().to_vec()),
        namespace: BoundedVec("SXT_SYSTEM_STAKING".as_bytes().to_vec()),
    });
    let batch_id: BatchId = BoundedVec("staking_batch_id5".as_bytes().to_vec());

    let row_data: Data = get_staking_message(eth_wallet, ammount);

    // Create the submit_data transaction
    let tx = tx()
        .indexing()
        .submit_data(staking_table, batch_id, row_data);

    let nonce_value = nonce.load(Ordering::Acquire);
    info!("Submitting transaction with nonce: {}", nonce_value);
    let tx_params = Params::new().nonce(nonce_value).build();
    nonce.fetch_add(1u64, Ordering::SeqCst);

    match client
        .tx()
        .sign_and_submit_then_watch(&tx, keypair, tx_params)
        .await
    {
        Ok(progress) => match progress.wait_for_finalized_success().await {
            Ok(_) => info!("Transaction finalized successfully"),
            Err(e) => error!("Staking Data Submission failed: {}", e),
        },
        Err(e) => error!("Failed to sign and send transaction: {}", e),
    }
}

/// Send to substrate
async fn send_session_keys(
    eth_wallet: &str,
    session_keys: &str,
    client: &Arc<Mutex<OnlineClient<PolkadotConfig>>>,
    keypair: &Keypair,
    nonce: Arc<AtomicU64>,
) {
    let client = client.lock().await;

    let staking_table = Table::from(TableIdentifier {
        name: BoundedVec("MESSAGE".as_bytes().to_vec()),
        namespace: BoundedVec("SXT_SYSTEM_STAKING".as_bytes().to_vec()),
    });
    let batch_id: BatchId = BoundedVec("messaging_batch_id5".as_bytes().to_vec());

    let row_data: Data = get_session_keys_message(eth_wallet, session_keys);

    // Create the submit_data transaction
    let tx = tx()
        .indexing()
        .submit_data(staking_table, batch_id, row_data);

    let nonce_value = nonce.load(Ordering::Acquire);
    info!("Submitting transaction with nonce: {}", nonce_value);
    let tx_params = Params::new().nonce(nonce_value).build();
    nonce.fetch_add(1u64, Ordering::SeqCst);

    match client
        .tx()
        .sign_and_submit_then_watch(&tx, keypair, tx_params)
        .await
    {
        Ok(progress) => match progress.wait_for_finalized_success().await {
            Ok(_) => info!("Transaction finalized successfully"),
            Err(e) => error!("Session Key Registration Data Submission failed: {}", e),
        },
        Err(e) => error!("Failed to sign and send transaction: {}", e),
    }
}

/// Sends a submit_data message for a staking transaction and a key registration transaction
/// as if they had been bridged from the Sepolia testnet
/// private_key: The private key of the indexer to impersonate
/// rpc: The rpc url of the substrate RPC node we are sending to
/// session_keys: The session keys from the rotateKeys call for the test validator node
/// eth_wallet: The eth wallet we should report as the original transactor
pub(crate) async fn test_staking(
    private_key: &str,
    rpc: &Url,
    session_keys: &str,
    eth_wallet: &str,
) -> anyhow::Result<()> {
    let signer = Keypair::from_uri(&SecretUri::from_str(private_key)?)?;
    let unit = U256::from(1_000_000_000_000_000_000_000_000_000u128); // 18 Zeros
    let sxt_count = U256::from(1_000);
    let test_amount = unit * sxt_count; // 1000 SXT

    let subxt_client = common::create_subxt_client(rpc).await?;

    let id = signer.public_key().to_account_id();
    let nonce_start = common::get_starting_nonce(subxt_client.clone(), id).await;
    let nonce = Arc::new(AtomicU64::new(nonce_start));

    send_staking_message(
        eth_wallet,
        test_amount,
        &subxt_client,
        &signer,
        nonce.clone(),
    )
    .await;
    send_session_keys(
        eth_wallet,
        session_keys,
        &subxt_client,
        &signer,
        nonce.clone(),
    )
    .await;

    Ok(())
}
