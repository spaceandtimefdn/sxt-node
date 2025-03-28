use serde::{Deserialize, Serialize};
use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::smartcontracts::{
    Contract,
    ContractDetails,
    ImplementationContract,
    NormalContract,
    ProxyContract,
};
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_runtime::Runtime;
use utoipa::ToSchema;

use crate::utils::{convert_event_details_to_chain, string_to_source};

/// Represents the execution status of a transaction.
///
/// This struct provides information about whether the transaction was successful
/// and, if applicable, additional details regarding its execution.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TxExecutionStatus {
    /// Indicates whether the transaction was executed successfully.
    pub success: bool,
    /// Additional details about the execution, such as error messages if the transaction failed.
    pub details: Option<String>,
}

/// Represents the status response for a transaction.
///
/// This struct provides details about a transaction's state, including whether it has been
/// validated, finalized, or encountered errors.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TxStatusResponse {
    /// Indicates whether the query was processed successfully.
    pub success: bool,
    /// Detailed information about the transaction's status.
    pub status: TxStatusDetails,
}

/// Represents a request to add a smart contract to the indexing system.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
/// A standard smart contract.
pub enum AddContractRequest {
    /// A normal contract
    Normal {
        /// The source blockchain network.
        source: String,
        /// The contract's address on the source chain.
        address: String,
        /// The optional ABI (Application Binary Interface) of the contract.
        abi: Option<String>,
        /// The block number from which to start indexing.
        starting_block: Option<u64>,
        /// The schema or DDL statement for the contract data.
        target_schema: Option<String>,
        /// A human-readable name for the contract.
        contract_name: Option<String>,
        /// List of event mappings (origin event -> target table).
        events: Vec<EventMapping>,
        /// The DDL statement correpsonding to this smart contract
        ddl_statement: Option<String>,
        /// The tables to create along with this smartcontract
        tables: Vec<TableRequest>,
    },
    /// A proxy contract that points to an implementation contract.
    Proxy {
        /// The source blockchain network.
        source: String,
        /// The proxy contract's address.
        address: String,
        /// The address of the contract's implementation.
        implementation_address: String,
        /// The optional ABI of the contract.
        abi: Option<String>,
        /// The block number from which to start indexing.
        starting_block: Option<u64>,
        /// The schema or DDL statement for the contract data.
        target_schema: Option<String>,
        /// A human-readable name for the contract.
        contract_name: Option<String>,
        /// List of event mappings (origin event -> target table).
        events: Vec<EventMapping>,
        /// The DDL statement correpsonding to this smart contract
        ddl_statement: Option<String>,
        /// The tables to create along with this smartcontract
        tables: Vec<TableRequest>,
    },
}

/// Represents an event mapping for a smart contract.
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventMapping {
    /// The event name.
    pub name: String,
    /// The event signature.
    pub signature: String,
    /// The target table name.
    pub table: String,
}

/// Represents a request to remove a smart contract from the indexing system.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub struct RemoveContractRequest {
    /// The source blockchain network.
    pub source: String,
    /// The contract's address on the source chain.
    pub address: String,
}

/// Represents a request to retrieve a smart contract's details.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub struct GetSmartcontractRequest {
    /// The source blockchain network.
    pub source: String,
    /// The contract's address on the source chain.
    pub address: String,
}

/// Represents the response format when retrieving a contract's details.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetContractResponse {
    /// Indicates whether the request was successful.
    pub success: bool,
    /// An optional error message in case of failure.
    pub err_msg: Option<String>,
    /// The retrieved contract details, if found.
    pub contract: Option<ApiContract>,
}

/// Generic API response structure.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse {
    /// Indicates whether the request was successful.
    pub success: bool,
    /// An optional error message in case of failure.
    pub err_msg: Option<String>,
    /// The transaction hash associated with the request, if applicable.
    pub tx_hash: Option<String>,
}

/// Represents a request to create tables for storing indexed data.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTableRequest {
    /// Schema name
    pub schema_name: String,
    /// Schema level DDL statement
    pub ddl_statement: String,
    /// A list of tables to be created.
    pub tables: Vec<TableRequest>,
    /// The table name.
    pub table_name: String,
    /// The table type
    pub table_type: TableType,
}

/// Represents an individual table entry inside a `CreateTableRequest`.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TableRequest {
    /// table type
    pub table_type: TableType,
    /// The table name.
    pub table_name: String,
    /// The namespace of the table.
    pub schema_name: String,
    /// The Data Definition Language (DDL) statement defining tnkhe table schema.
    pub ddl_statement: String,
    /// commitment data
    pub commitment: String,
    /// snapshot location
    pub snapshot_url: String,
    /// commitment scheme
    pub commitment_scheme: CommitmentScheme,
}

/// Represents quorum settings for table creation.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuorumSize {
    /// The required number of public approvals.
    pub public: Option<u8>,
    /// The required number of privileged approvals.
    pub privileged: Option<u8>,
}

/// Represents the API response format for retrieving contract details.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ApiContract {
    /// A standard smart contract.
    Normal {
        /// The source blockchain network.
        source: String,
        /// The contract's address on the source chain.
        address: String,
        /// The optional ABI of the contract.
        abi: Option<String>,
        /// The block number from which to start indexing.
        starting_block: Option<u64>,
        /// The schema or DDL statement for the contract data.
        target_schema: Option<String>,
        /// A human-readable name for the contract.
        contract_name: Option<String>,
        /// A list of events that this smart contract emits that we are interested in indexing
        events: Vec<EventMapping>,
        /// The DDL statement correpsonding to this smart contract
        ddl_statement: Option<String>,
    },
    /// A proxy contract that points to an implementation contract.
    Proxy {
        /// The source blockchain network.
        source: String,
        /// The proxy contract's address.
        address: String,
        /// The address of the contract's implementation.
        implementation_address: String,
        /// The optional ABI of the contract.
        abi: Option<String>,
        /// The block number from which to start indexing.
        starting_block: Option<u64>,
        /// The schema or DDL statement for the contract data.
        target_schema: Option<String>,
        /// A human-readable name for the contract.
        contract_name: Option<String>,
        /// A list of events that this smart contract emits that we are interested in indexing
        events: Vec<EventMapping>,
        /// The DDL statement correpsonding to this smart contract
        ddl_statement: Option<String>,
    },
}

impl AddContractRequest {
    /// convert a json request to a contract type
    pub fn to_contract(self) -> (Vec<TableRequest>, Contract) {
        match self {
            AddContractRequest::Normal {
                source,
                address,
                abi,
                starting_block,
                target_schema,
                contract_name,
                events,
                tables,
                ..
            } => {
                (
                    tables,
                    Contract::Normal(NormalContract {
                        details: ContractDetails {
                            source: string_to_source(&source),
                            address: BoundedVec(address.into_bytes()), // Convert String -> BoundedVec<u8>
                            abi: abi.map(|s| BoundedVec(s.into_bytes())),
                            starting_block,
                            target_schema: target_schema.map(|s| BoundedVec(s.into_bytes())),
                            contract_name: contract_name.map(|n| BoundedVec(n.into_bytes())),
                            event_details: convert_event_details_to_chain(events),
                        },
                    }),
                )
            }

            AddContractRequest::Proxy {
                source,
                address,
                implementation_address,
                abi,
                starting_block,
                target_schema,
                contract_name,
                events,
                tables,
                ..
            } => {
                (
                    tables,
                    Contract::Proxy(ProxyContract {
                        details: ContractDetails {
                            source: string_to_source(&source),
                            address: BoundedVec(address.into_bytes()),
                            abi: abi.map(|s| BoundedVec(s.into_bytes())),
                            starting_block,
                            target_schema: target_schema.map(|s| BoundedVec(s.into_bytes())),
                            contract_name: contract_name.map(|n| BoundedVec(n.into_bytes())),
                            event_details: convert_event_details_to_chain(events.clone()),
                        },
                        implementation: ImplementationContract {
                            details: ContractDetails {
                                source: string_to_source(&source),
                                address: BoundedVec(implementation_address.into_bytes()), // Implementation contract address
                                abi: None, // ABI is only stored for the primary contract
                                starting_block: None,
                                target_schema: None,
                                contract_name: None,
                                event_details: convert_event_details_to_chain(events),
                            },
                        },
                    }),
                )
            }
        }
    }
}

/// Represents a request to drop a table.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DropTableRequest {
    /// The schema name in which the table exists.
    pub schema_name: String,
    /// The table name to drop.
    pub table_name: String,
    /// The source blockchain network.
    pub source: String,
    /// The indexing mode.
    pub mode: String,
    /// Table type
    pub table_type: TableType,
}

/// Response structure for retrieving multiple smart contracts.
///
/// This struct is returned when querying for all smart contracts associated with a given source.
///
/// # Fields
/// - `success` (bool): Indicates whether the query was successfully processed.
/// - `err_msg` (Option<String>): An optional error message if the request fails.
/// - `contracts` (Vec<ApiContract>): A list of retrieved smart contracts.
///
/// # Example Response
/// ```json
/// {
///   "success": true,
///   "errMsg": null,
///   "contracts": [
///     {
///       "source": "Ethereum",
///       "address": "0x1234567890abcdef1234567890abcdef12345678",
///       "contractName": "TestContract",
///       "startingBlock": 123456,
///       "abi": null,
///       "events": [
///         {
///           "name": "Transfer",
///           "signature": "Transfer(indexed address from, indexed address to, uint256 value)",
///           "table": "event_transfer"
///         }
///       ]
///     }
///   ]
/// }
/// ```
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetContractsResponse {
    /// Indicates whether the query was successful.
    pub success: bool,
    /// An optional error message in case of failure.
    pub err_msg: Option<String>,
    /// The list of retrieved smart contracts.
    pub contracts: Vec<ApiContract>,
}

/// Provides detailed information about the status of a blockchain transaction.
///
/// This struct contains various transaction state indicators, including whether it has been finalized,
/// broadcasted, dropped, or encountered an error.
///
/// # Fields
/// - `validated` (bool): Whether the transaction has been validated.
/// - `no_longer_in_best_block` (bool): Indicates if the transaction was removed from the best block.
/// - `broadcasted_peers` (Option<u32>): The number of peers the transaction was broadcasted to.
/// - `in_best_block` (Option<String>): The block hash where the transaction was included.
/// - `finalized_in_block` (Option<String>): The block hash where the transaction was finalized.
/// - `dropped_message` (Option<String>): If applicable, a message explaining why the transaction was dropped.
/// - `invalid_message` (Option<String>): If applicable, a message explaining why the transaction was deemed invalid.
/// - `error_message` (Option<String>): If applicable, a message describing any error encountered during execution.
///
/// # Example Response
/// ```json
/// {
///   "validated": true,
///   "noLongerInBestBlock": false,
///   "broadcastedPeers": 3,
///   "inBestBlock": "0xabc123...",
///   "finalizedInBlock": "0xdef456...",
///   "droppedMessage": null,
///   "invalidMessage": null,
///   "errorMessage": null
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TxStatusDetails {
    /// Indicates whether the transaction has been validated.
    pub validated: bool,
    /// Specifies if the transaction is no longer in the best block.
    pub no_longer_in_best_block: bool,
    /// The number of peers the transaction was broadcasted to.
    pub broadcasted_peers: Option<u32>,
    /// The block hash where the transaction was included.
    pub in_best_block: Option<String>,
    /// The block hash where the transaction was finalized.
    pub finalized_in_block: Option<String>,
    /// If applicable, a message explaining why the transaction was dropped.
    pub dropped_message: Option<String>,
    /// If applicable, a message explaining why the transaction was deemed invalid.
    pub invalid_message: Option<String>,
    /// If applicable, a message describing any error encountered during execution.
    pub error_message: Option<String>,
}

/// Represents the type of a table being created or referenced within the translation layer.
///
/// This enum categorizes tables based on their intended usage and ownership model.
/// It is used to determine how data should be interpreted, stored, or indexed
/// across different subsystems.
///
/// # Variants
/// - `CoreBlockchain`: A default system table directly tied to the core blockchain state.
/// - `SCI`: A table used for indexing smart contract events and data (Smart Contract Indexing).
/// - `Community`: A table managed or curated by the community or external contributors.
#[derive(Debug, Serialize, Deserialize, ToSchema, Default, Clone)]
pub enum TableType {
    /// Core Blockchain table — default system tables that reflect on-chain primitives.
    #[default]
    CoreBlockchain,

    /// Smart Contract Indexing — used to store and query smart contract-related data.
    SCI,

    /// Community Owned Table — tables created and managed by users or external tooling.
    Community,
}

use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::TableType as RuntimeTableType;

impl From<TableType> for RuntimeTableType {
    fn from(t: TableType) -> Self {
        match t {
            TableType::CoreBlockchain => RuntimeTableType::CoreBlockchain,
            TableType::SCI => RuntimeTableType::SCI,
            TableType::Community => RuntimeTableType::Community,
        }
    }
}

impl From<RuntimeTableType> for TableType {
    fn from(rt: RuntimeTableType) -> Self {
        match rt {
            RuntimeTableType::CoreBlockchain => TableType::CoreBlockchain,
            RuntimeTableType::SCI => TableType::SCI,
            RuntimeTableType::Community => TableType::Community,
            _ => TableType::CoreBlockchain,
        }
    }
}

/// Parsable commitment schemes
#[derive(Debug, Serialize, Deserialize, ToSchema, Default, Clone)]
pub enum CommitmentScheme {
    /// HyperKzg
    HyperKzg,
    /// dynamic dory
    #[default]
    DynamicDory,
}
