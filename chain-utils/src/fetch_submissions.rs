use std::str::from_utf8;

use anyhow::{anyhow, Result};
use log::{error, info};
use subxt::utils::H256;
use sxt_core::sxt_chain_runtime::api::indexing::calls::types::SubmitData;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::TableIdentifier;

use crate::common::create_subxt_client;
use crate::print_batch::print_batch;

/// Retrieves submit data extrinsic for a given block hash and prints the record batches
pub async fn fetch_submissions(hash: H256, rpc: &url::Url) -> Result<()> {
    let client = create_subxt_client(rpc).await?;
    let client = client.lock().await;

    info!("Fetching submit_data extrinsics from block {:?}", hash);

    let extrinsics = client.blocks().at(hash).await?.extrinsics().await?;

    extrinsics.iter().for_each(|e| {
        if let Ok(Some(submission)) = e.as_extrinsic::<SubmitData>() {
            match print_submission(&submission) {
                Ok(_) => {}
                Err(_) => {
                    error!("Failed to print submission");
                }
            }
        }
    });

    Ok(())
}

/// Takes a TableIdentifier object from the chain and returns a String for convenient logging
fn table_to_str(table: &TableIdentifier) -> String {
    let name = from_utf8(&table.name.0).unwrap();
    let namespace = from_utf8(&table.namespace.0).unwrap();
    format!("{}.{}", namespace, name)
}

/// Print interesting data about the submission extrinsic
fn print_submission(submission: &SubmitData) -> Result<()> {
    let hex = hex::encode(&submission.data.0);
    println!("Submission to table {:?}", table_to_str(&submission.table));
    print_batch(hex.as_str())
}
