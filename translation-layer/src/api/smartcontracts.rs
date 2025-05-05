use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::smartcontracts::Contract;

use crate::model::{
    AddContractRequest,
    ApiContract,
    ApiResponse,
    GetContractResponse,
    GetContractsResponse,
    RemoveContractRequest,
    TableRequest,
};
use crate::state::TranslationLayerState;
use crate::table_builder::TableCreator;
use crate::utils::{
    extract_param,
    internal_server_error,
    map_contract_to_api,
    not_found,
    string_to_source,
};

/// Submits a transaction to add a smart contract to the indexing system.
///
/// This endpoint takes a smart contract configuration, converts it into the appropriate runtime format,
/// and submits it as an extrinsic to the blockchain.
///
/// # Request Body
/// - `source` (String): The source blockchain network.
/// - `address` (String): The contract's address on the source chain.
/// - `abi` (Option<String>): The optional Application Binary Interface (ABI) of the contract.
/// - `starting_block` (Option<u64>): The block number from which to start indexing.
/// - `target_schema` (Option<String>): The schema or DDL statement for contract data.
/// - `contract_name` (Option<String>): A human-readable name for the contract.
/// - `events` (Vec<EventMapping>): List of event mappings (origin event → target table).
/// - `ddl_statement` (Option<String>): The DDL statement corresponding to the smart contract.
///
/// # Responses
/// - **200 OK**: Smart contract successfully added.
/// - **400 BAD REQUEST**: Invalid request parameters.
/// - **500 INTERNAL SERVER ERROR**: Transaction submission failed.
///
/// # Example Usage
/// ```sh
/// curl -X POST "http://127.0.0.1:3000/add_smartcontract" -H "Content-Type: application/json" -d '{...}'
/// ```
#[utoipa::path(post, path = "/add_smartcontract", tag = "add-smartcontract",
  request_body = AddContractRequest,
  responses(
      (status = 200, description = "Smart contract added successfully", body = ApiResponse),
      (status = 400, description = "Invalid request", body = ApiResponse),
      (status = 500, description = "Internal server error", body = ApiResponse)
  ))]
pub async fn add_smartcontract(
    State(state): State<Arc<TranslationLayerState>>, // Get shared API instance
    Json(request): Json<AddContractRequest>,
) -> Json<ApiResponse> {
    let (tables, contract) = match request.try_into() {
        Ok(v) => v,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                err_msg: Some(format!("Invalid contract input: {e}")),
                tx_hash: None,
            });
        }
    };

    let mut table_creator = TableCreator::new();

    for table in tables.iter() {
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

    let tables = BoundedVec(table_creator.tables());
    let tx = sxt_chain_runtime::api::tx()
        .smartcontracts()
        .add_smartcontract(contract, tables);

    let submitter_opt = match state.network {
        crate::state::Network::Mainnet => state.mainnet_submitter.as_ref(),
        crate::state::Network::Testnet => state.testnet_submitter.as_ref(),
    };

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

/// Submits a transaction to remove a smart contract from the indexing system.
///
/// This endpoint allows for the removal of an indexed smart contract from the blockchain storage.
///
/// # Request Body
/// - `source` (String): The source blockchain network.
/// - `address` (String): The contract's address on the source chain.
///
/// # Responses
/// - **200 OK**: Smart contract successfully removed.
/// - **400 BAD REQUEST**: Invalid request parameters.
/// - **500 INTERNAL SERVER ERROR**: Transaction submission failed.
///
/// # Example Usage
/// ```sh
/// curl -X POST "http://127.0.0.1:3000/remove_smartcontract" -H "Content-Type: application/json" -d '{...}'
/// ```
#[utoipa::path(post, path = "/remove_smartcontract", tag = "remove-smartcontract",
  request_body = RemoveContractRequest,
  responses(
      (status = 200, description = "Smart contract removed successfully", body = ApiResponse),
      (status = 400, description = "Invalid request", body = ApiResponse),
      (status = 500, description = "Internal server error", body = ApiResponse)
  ))]
pub async fn remove_smartcontract(
    State(state): State<Arc<TranslationLayerState>>, // Get shared API instance
    Json(RemoveContractRequest { source, address }): Json<RemoveContractRequest>,
) -> Json<ApiResponse> {
    let source = string_to_source(&source);
    let address = BoundedVec(address.into_bytes().to_vec());

    let tx = sxt_chain_runtime::api::tx()
        .smartcontracts()
        .remove_smartcontract(source, address);

    let submitter_opt = match state.network {
        crate::state::Network::Mainnet => state.mainnet_submitter.as_ref(),
        crate::state::Network::Testnet => state.testnet_submitter.as_ref(),
    };

    let Some(submitter) = submitter_opt else {
        return Json(ApiResponse {
            success: false,
            err_msg: Some("TxSubmitter not configured for this network".into()),
            tx_hash: None,
        });
    };

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

/// Retrieves the details of a specific smart contract.
///
/// This endpoint queries the blockchain storage for a smart contract by its source and address.
///
/// # Query Parameters
/// - `source` (String): The blockchain network of the smart contract.
/// - `address` (String): The address of the smart contract.
///
/// # Responses
/// - **200 OK**: Successfully retrieved the smart contract details.
/// - **404 NOT FOUND**: Smart contract not found.
/// - **500 INTERNAL SERVER ERROR**: Error accessing blockchain storage.
///
/// # Example Usage
/// ```sh
/// curl -X GET "http://127.0.0.1:3000/get_smartcontract?source=Ethereum&address=0x123..."
/// ```
#[utoipa::path(
      get,
      path = "/get_smartcontract",
      tag = "/get-smartcontract",
      params(
          ("source" = String, Query, description = "Source of the smart contract"),
          ("address" = String, Query, description = "Address of the smart contract")
      ),
      responses(
          (status = 200, description = "Successfully retrieved contract", body = GetContractResponse),
          (status = 404, description = "Contract not found", body = ApiResponse),
          (status = 500, description = "Internal server error", body = ApiResponse)
      )
  )]
pub async fn get_smartcontract(
    State(state): State<Arc<TranslationLayerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GetContractResponse>, (StatusCode, Json<ApiResponse>)> {
    let source = extract_param(&params, "source")?;
    let address = extract_param(&params, "address")?;

    let source = string_to_source(&source);
    let address = BoundedVec(address.as_bytes().to_vec());

    let query = sxt_chain_runtime::api::storage()
        .smartcontracts()
        .contract_storage(&source, &address);

    let submitter_opt = match state.network {
        crate::state::Network::Mainnet => state.mainnet_submitter.as_ref(),
        crate::state::Network::Testnet => state.testnet_submitter.as_ref(),
    };

    let Some(submitter) = submitter_opt else {
        return Err(internal_server_error("tx submitter improperly initialized"));
    };

    let storage = submitter
        .lock()
        .await
        .client
        .lock()
        .await
        .storage()
        .at_latest()
        .await
        .map_err(|err| internal_server_error("Failed to access storage"))?;

    let contract = storage
        .fetch(&query)
        .await
        .map_err(|_| not_found("Contract not found"))?
        .ok_or_else(|| not_found("Contract not found"))?;

    let api_contract = map_contract_to_api(contract, &source, address);

    Ok(Json(GetContractResponse {
        success: true,
        err_msg: None,
        contract: Some(api_contract),
    }))
}

/// Retrieves a list of all smart contracts indexed for a given blockchain source.
///
/// This endpoint iterates over the blockchain storage to fetch all smart contracts stored for a specified source.
///
/// # Query Parameters
/// - `source` (String): The blockchain network whose smart contracts should be retrieved.
///
/// # Responses
/// - **200 OK**: Successfully retrieved the list of contracts.
/// - **404 NOT FOUND**: No contracts found for the given source.
/// - **500 INTERNAL SERVER ERROR**: Error accessing blockchain storage.
///
/// # Example Usage
/// ```sh
/// curl -X GET "http://127.0.0.1:3000/get_smartcontracts?source=Ethereum"
/// ```
#[utoipa::path(
    get,
    path = "/get_smartcontracts",
    tag = "get-smartcontracts",
    params(
        ("source" = String, Query, description = "Source of the smart contracts")
    ),
    responses(
        (status = 200, description = "Successfully retrieved contracts", body = GetContractsResponse),
        (status = 404, description = "No contracts found for the given source", body = ApiResponse),
        (status = 500, description = "Internal server error", body = ApiResponse)
    )
)]
pub async fn get_smartcontracts(
    State(state): State<Arc<TranslationLayerState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<GetContractsResponse>, (StatusCode, Json<ApiResponse>)> {
    let source = extract_param(&params, "source")?;
    let source = string_to_source(&source);

    // sxt_chain_runtime::api::storage().smartcontracts().contract_storage_iter();

    // Query to iterate over all contracts for the given source
    let query = sxt_chain_runtime::api::storage()
        .smartcontracts()
        .contract_storage_iter1(&source);

    let submitter_opt = match state.network {
        crate::state::Network::Mainnet => state.mainnet_submitter.as_ref(),
        crate::state::Network::Testnet => state.testnet_submitter.as_ref(),
    };

    let Some(submitter) = submitter_opt else {
        return Err(internal_server_error("tx submitter improperly initialized"));
    };

    let storage = submitter
        .lock()
        .await
        .client
        .lock()
        .await
        .storage()
        .at_latest()
        .await
        .map_err(|err| internal_server_error("Failed to access storage"))?;

    let mut contract_stream = storage.iter(query).await.map_err(|err| {
        internal_server_error(&format!("Failed to get storage iterator: {}", err))
    })?;

    let mut contracts: Vec<ApiContract> = vec![];

    while let Some(Ok(contract)) = contract_stream.next().await {
        let value = contract.value;
        let address = match value {
            Contract::Normal(ref normal) => normal.details.address.0.clone(),
            Contract::Proxy(ref proxy) => proxy.details.address.0.clone(),
        };
        contracts.push(map_contract_to_api(value, &source, BoundedVec(address)));
    }

    if contracts.is_empty() {
        return Err(not_found("No contracts found for the given source"));
    }

    Ok(Json(GetContractsResponse {
        success: true,
        err_msg: None,
        contracts,
    }))
}
