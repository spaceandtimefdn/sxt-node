//! api handlers for extrinsics
use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use subxt::tx::TxStatus;
use sxt_core::sxt_chain_runtime;

use crate::model::{ApiResponse, TxExecutionStatus, TxStatusDetails, TxStatusResponse};
use crate::state::TranslationLayerState;
use crate::utils::{
    bad_request,
    decode_system_module_error,
    internal_server_error,
    not_found,
    parse_h256_from_hex,
};

/// Retrieves the execution status of a transaction within a specific block.
///
/// This endpoint fetches the extrinsics from the specified block and verifies if the given transaction (`tx_hash`)
/// was included in that block. If found, it checks for any failure events associated with the transaction.
///
/// # Query Parameters
/// - `tx_hash` (String): The transaction hash to check.
/// - `block_hash` (String): The hash of the block where the transaction is expected to be found.
///
/// # Responses
/// - **200 OK**: The transaction execution status is retrieved successfully.
/// - **400 BAD REQUEST**: Invalid request parameters (e.g., missing or incorrectly formatted `tx_hash` or `block_hash`).
/// - **404 NOT FOUND**: The transaction was not found in the given block.
/// - **500 INTERNAL SERVER ERROR**: An error occurred while fetching block data or extrinsics.
///
/// # Example Usage
/// ```sh
/// curl -X GET "http://127.0.0.1:3000/get_extrinsic_status_in_block?tx_hash=0x123...&block_hash=0xabc..."
/// ```
#[utoipa::path(
        get,
        path = "/get_extrinsic_status_in_block",
        tag = "get-extrinsic-status-in-block",
        params(
            ("tx_hash" = String, Query, description = "Transaction hash"),
            ("block_hash" = String, Query, description = "Block hash")
        ),
        responses(
            (status = 200, description = "Transaction status retrieved", body = TxExecutionStatus),
            (status = 400, description = "Invalid request parameters", body = ApiResponse),
            (status = 404, description = "Transaction not found in the block", body = ApiResponse),
            (status = 500, description = "Internal server error", body = ApiResponse)
        )
    )]
pub async fn get_extrinsic_status_in_block(
    State(state): State<Arc<TranslationLayerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<TxExecutionStatus>, (StatusCode, Json<ApiResponse>)> {
    // Parse tx_hash
    let tx_hash = params
        .get("tx_hash")
        .ok_or_else(|| bad_request("Missing required query parameter: tx_hash"))?;
    let tx_hash = parse_h256_from_hex(tx_hash)
        .map_err(|e| bad_request(&format!("Invalid tx_hash format: {}", e)))?;

    // Parse block_hash
    let block_hash = params
        .get("block_hash")
        .ok_or_else(|| bad_request("Missing required query parameter: block_hash"))?;
    let block_hash = parse_h256_from_hex(block_hash)
        .map_err(|e| bad_request(&format!("Invalid block_hash format: {}", e)))?;

    // Fetch block at given hash
    let block_at_hash = state
        .client
        .blocks()
        .at(block_hash)
        .await
        .map_err(|e| internal_server_error(&format!("Subxt error fetching block: {}", e)))?;

    // Fetch extrinsics
    let extrinsics = block_at_hash
        .extrinsics()
        .await
        .map_err(|e| internal_server_error(&format!("Subxt error fetching extrinsics: {}", e)))?;

    let metadata = state.client.metadata();

    if let Some(extrinsic) = extrinsics.iter().find(|ext| ext.hash() == tx_hash) {
        let events = extrinsic
            .events()
            .await
            .map_err(|e| internal_server_error(&format!("Subxt error fetching events: {}", e)))?;

        for event in events.iter() {
            let event = event
                .map_err(|e| internal_server_error(&format!("Failed to parse event: {}", e)))?;
            if let Ok(Some(extrinsic_failed)) =
                event.as_event::<sxt_chain_runtime::api::system::events::ExtrinsicFailed>()
            {
                if let Some(decoded_error) =
                    decode_system_module_error(&extrinsic_failed.dispatch_error, &metadata)
                {
                    return Ok(Json(TxExecutionStatus {
                        success: false,
                        details: Some(format!("Extrinsic execution failed: {}", decoded_error)),
                    }));
                }
            }
        }

        // If no failure event was found, return success
        return Ok(Json(TxExecutionStatus {
            success: true,
            details: None,
        }));
    }

    // If no matching extrinsic found, return 404
    Err(not_found("Transaction not found in the block"))
}

/// Retrieves the status of a transaction within the blockchain.
///
/// This endpoint queries the transaction progress based on its `tx_hash`. It returns detailed status information,
/// including whether the transaction has been validated, broadcasted, included in a block, finalized, or encountered any errors.
///
/// # Query Parameters
/// - `tx_hash` (String): The transaction hash to check.
///
/// # Responses
/// - **200 OK**: The transaction status is retrieved successfully.
/// - **404 NOT FOUND**: The transaction was not found.
/// - **500 INTERNAL SERVER ERROR**: An error occurred while fetching transaction status.
///
/// # Example Usage
/// ```sh
/// curl -X GET "http://127.0.0.1:3000/get_extrinsic_status?tx_hash=0x123..."
/// ```
///
/// # Returned Transaction Status Fields:
/// - `validated`: Whether the transaction has been validated.
/// - `broadcasted_peers`: Number of peers the transaction was broadcasted to.
/// - `in_best_block`: Block hash where the transaction was included.
/// - `finalized_in_block`: Block hash where the transaction was finalized.
/// - `dropped_message`: If the transaction was dropped, the reason.
/// - `invalid_message`: If the transaction was marked invalid, the reason.
/// - `error_message`: If the transaction encountered an error, details of the error.
#[utoipa::path(
    get,
    path = "/get_extrinsic_status",
    tag = "get-extrinsic-status",
    params(("tx_hash" = String, Query, description = "Transaction hash")),
    responses(
        (status = 200, description = "Transaction status retrieved", body = TxStatusResponse),
        (status = 404, description = "Transaction not found", body = ApiResponse),
        (status = 500, description = "Internal server error", body = ApiResponse)
    )
)]
pub async fn get_extrinsic_status(
    State(state): State<Arc<TranslationLayerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<TxStatusResponse>, (StatusCode, Json<ApiResponse>)> {
    let tx_hash = params.get("tx_hash").ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                err_msg: Some("Missing required query parameter: tx_hash".to_string()),
                tx_hash: None,
            }),
        )
    })?;

    match state.tx_db.get_history(tx_hash).await {
        Some(status_list) => {
            let mut tx_status = TxStatusDetails {
                validated: false,
                no_longer_in_best_block: false,
                broadcasted_peers: None,
                in_best_block: None,
                finalized_in_block: None,
                dropped_message: None,
                invalid_message: None,
                error_message: None,
            };

            for status in status_list.iter() {
                match **status {
                    TxStatus::Broadcasted { num_peers } => {
                        tx_status.broadcasted_peers = Some(num_peers);
                    }
                    TxStatus::InBestBlock(ref block) => {
                        tx_status.in_best_block = Some(format!("{:#x}", block.block_hash()));
                    }
                    TxStatus::InFinalizedBlock(ref block) => {
                        tx_status.finalized_in_block = Some(format!("{:#x}", block.block_hash()));
                    }
                    TxStatus::Dropped { ref message } => {
                        tx_status.dropped_message = Some(message.clone());
                    }
                    TxStatus::Invalid { ref message } => {
                        tx_status.invalid_message = Some(message.clone());
                    }
                    TxStatus::Error { ref message } => {
                        tx_status.error_message = Some(message.clone());
                    }
                    TxStatus::Validated => {
                        tx_status.validated = true;
                    }
                    TxStatus::NoLongerInBestBlock => {
                        tx_status.no_longer_in_best_block = true;
                    }
                }
            }

            Ok(Json(TxStatusResponse {
                success: true,
                status: tx_status,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse {
                success: false,
                err_msg: Some("Transaction not found".to_string()),
                tx_hash: None,
            }),
        )),
    }
}
