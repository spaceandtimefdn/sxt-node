use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::{
    SourceAndMode,
    TableIdentifier,
};

use crate::model::{ApiResponse, CreateTableRequest, DropTableRequest, TableRequest};
use crate::state::TranslationLayerState;
use crate::table_builder::TableCreator;
use crate::utils::{string_to_mode, string_to_source};

/// Submits a transaction to create a new table in the indexing system.
///
/// This endpoint constructs a table creation request and submits it as an extrinsic to the blockchain.
///
/// # Request Body
/// - `schema_name` (String): The schema namespace for the table.
/// - `ddl_statement` (String): The Data Definition Language (DDL) statement defining the table structure.
/// - `source` (String): The source blockchain network.
/// - `mode` (String): The indexing mode for the table.
/// - `tables` (Vec<TableRequest>): A list of tables to be created.
/// - `table_name` (String): The name of the table.
///
/// # Responses
/// - **200 OK**: Table successfully created.
/// - **400 BAD REQUEST**: Invalid request parameters.
/// - **500 INTERNAL SERVER ERROR**: Transaction submission failed.
///
/// # Example Usage
/// ```sh
/// curl -X POST "http://127.0.0.1:3000/create_table" -H "Content-Type: application/json" -d '{
///     "schemaName": "public",
///     "ddlStatement": "CREATE TABLE example_table (id SERIAL PRIMARY KEY, data TEXT);",
///     "source": "Ethereum",
///     "mode": "indexing",
///     "tables": [
///         {
///             "tableName": "example_table",
///             "schemaName": "public",
///             "ddlStatement": "CREATE TABLE example_table (id SERIAL PRIMARY KEY, data TEXT);"
///         }
///     ],
///     "tableName": "example_table"
/// }'
/// ```
#[utoipa::path(post, path = "/create_table", tag = "create-table",
    request_body = CreateTableRequest,
    responses(
        (status = 200, description = "Table created successfully", body = ApiResponse),
        (status = 400, description = "Invalid request", body = ApiResponse),
        (status = 500, description = "Internal server error", body = ApiResponse)
    ))]
pub async fn create_table(
    State(state): State<Arc<TranslationLayerState>>,
    Json(request): Json<Vec<TableRequest>>,
) -> Json<ApiResponse> {
    let mut table_creator = TableCreator::new();

    for table in request.iter() {
        let mut builder = table_creator
            .add_table()
            .identifier(&table.table_name, &table.schema_name)
            .ddl_statement(&table.ddl_statement)
            .table_type(table.table_type.clone().into())
            .source(table.source.clone());

        if let (Some(commitment_hex), Some(scheme), Some(snapshot)) = (
            &table.commitment,
            &table.commitment_scheme,
            &table.snapshot_url,
        ) {
            match hex::decode(commitment_hex.trim_start_matches("0x")) {
                Ok(decoded_commitment) => {
                    builder = builder
                        .commitment_scheme(scheme.clone())
                        .commitment(&decoded_commitment)
                        .snapshot_url(snapshot);
                }
                Err(_) => {
                    return Json(ApiResponse {
                        success: false,
                        err_msg: Some("Invalid hex commitment".into()),
                        tx_hash: None,
                    });
                }
            }
        }

        builder.add();
    }

    let tx = table_creator.build();

    // 🚦 Get the right submitter based on the current network
    let submitter_opt = match state.network {
        crate::state::Network::Mainnet => state.mainnet_submitter.as_ref(),
        crate::state::Network::Testnet => state.testnet_submitter.as_ref(),
    };

    // ❌ Return 500 if submitter is missing
    let Some(submitter) = submitter_opt else {
        return Json(ApiResponse {
            success: false,
            err_msg: Some("TxSubmitter not configured for this network".into()),
            tx_hash: None,
        });
    };

    // ✅ Lock and use submitter
    let mut submitter = submitter.lock().await;

    let (success, err_msg, tx_hash) = match submitter.submit_tx_get_hash(&tx).await {
        Ok(hash) => (true, None, Some(format!("{:#x}", hash))),
        Err(err) => (false, Some(format!("Error: {err}")), None),
    };

    Json(ApiResponse {
        success,
        err_msg,
        tx_hash,
    })
}

/// Submits a transaction to drop a table from the indexing system.
///
/// This endpoint allows for the removal of an indexed table from the blockchain storage.
///
/// # Request Body
/// - `schema_name` (String): The schema namespace of the table.
/// - `table_name` (String): The name of the table to be removed.
/// - `source` (String): The source blockchain network.
/// - `mode` (String): The indexing mode.
///
/// # Responses
/// - **200 OK**: Table successfully removed.
/// - **400 BAD REQUEST**: Invalid request parameters.
/// - **500 INTERNAL SERVER ERROR**: Transaction submission failed.
///
/// # Example Usage
/// ```sh
/// curl -X POST "http://127.0.0.1:3000/drop_table" -H "Content-Type: application/json" -d '{
///     "schemaName": "public",
///     "tableName": "example_table",
///     "source": "Ethereum",
///     "mode": "indexing"
/// }'
/// ```
#[utoipa::path(
    post,
    path = "/drop_table",
    tag = "drop-table",
    request_body = DropTableRequest,
    responses(
        (status = 200, description = "Table dropped successfully", body = ApiResponse),
        (status = 400, description = "Invalid request", body = ApiResponse),
        (status = 500, description = "Internal server error", body = ApiResponse)
    )
)]
pub async fn drop_table(
    State(state): State<Arc<TranslationLayerState>>,
    Json(request): Json<DropTableRequest>,
) -> Json<ApiResponse> {
    let tx = sxt_chain_runtime::api::tx().tables().drop_table(
        request.table_type.into(),
        TableIdentifier {
            name: BoundedVec(request.table_name.into()),
            namespace: BoundedVec(request.schema_name.into()),
        },
    );

    let submitter_opt = match state.network {
        crate::state::Network::Mainnet => state.mainnet_submitter.as_ref(),
        crate::state::Network::Testnet => state.testnet_submitter.as_ref(),
    };

    // ❌ Return 500 if submitter is missing
    let Some(submitter) = submitter_opt else {
        return Json(ApiResponse {
            success: false,
            err_msg: Some("TxSubmitter not configured for this network".into()),
            tx_hash: None,
        });
    };

    // ✅ Lock and use submitter
    let mut submitter = submitter.lock().await;

    let (success, err_msg, tx_hash) = match submitter.submit_tx_get_hash(&tx).await {
        Ok(hash) => (true, None, Some(format!("{:#x}", hash))),
        Err(err) => (false, Some(format!("Error: {err}")), None),
    };

    Json(ApiResponse {
        success,
        err_msg,
        tx_hash,
    })
}
