use attestation_tree::{AttestationTreeError, AttestationTreeProofError};
use jsonrpsee::types::ErrorObjectOwned;
use proof_of_sql_planner::PlannerError;
use snafu::Snafu;
use sxt_core::tables::TableIdentifierConversionError;

use crate::commitments::limits::{NUM_TABLES_LIMIT, PROOF_PLAN_SIZE_LIMIT, QUERY_SIZE_LIMIT};

/// The base error code used by the commitments RPCs.
const BASE_ERROR: i32 = 254000;

/// Errors that can occur in the commitments RPCs.
#[derive(Debug, Snafu)]
pub enum CommitmentsApiError {
    /// Proof plan size exceeds limit.
    #[snafu(display(
        "proof plan of size {proof_plan_size} exceeds limit {PROOF_PLAN_SIZE_LIMIT}"
    ))]
    ProofPlanSizeLimit {
        /// The actual proof plan size.
        proof_plan_size: usize,
    },
    /// Failed to deserialize proof plan.
    #[snafu(display("failed to deserialize proof plan: {source}"))]
    DeserializeProofPlan {
        /// The source bincode error.
        source: bincode::error::DecodeError,
    },
    /// Failed to convert table identifier from proof-of-sql to sxt-chain type.
    #[snafu(
        display(
            "failed to convert table identifier from proof-of-sql to sxt-chain type: {source}"
        ),
        context(false)
    )]
    TableIdentifierConversion {
        /// The source conversion error.
        source: TableIdentifierConversionError,
    },
    /// Number of tables exceeds limit.
    #[snafu(display("query against {num_tables} tables exceeds limit of {NUM_TABLES_LIMIT}"))]
    NumTablesLimit {
        /// The actual table count.
        num_tables: usize,
    },
    /// Failed to query storage.
    #[snafu(display("failed to query storage: {source}"), context(false))]
    Storage {
        /// The source substrate error.
        source: sp_blockchain::Error,
    },
    /// Commitment does not exist in storage.
    #[snafu(display("commitment does not exist in storage"))]
    NoSuchCommitment,
    /// Failed to decode commitment in storage.
    #[snafu(display("failed to decode commitment in storage"), context(false))]
    CommitmentDecode {
        /// The source codec error.
        source: codec::Error,
    },
    /// Failed to create attestation tree.
    #[snafu(display("failed to create attestation tree: {source}"), context(false))]
    AttestationTree {
        /// The source attestation tree error.
        source: AttestationTreeError,
    },
    /// Failed to generate merkle proof for commitments.
    #[snafu(
        display("failed to generate merkle proof for commitment: {source}"),
        context(false)
    )]
    AttestationTreeProof { source: AttestationTreeProofError },
    /// Staking contract info is not defined in storage.
    #[snafu(display("staking contract info is not defined in storage"))]
    NoStakingContract,
    /// Query size exceeds limit.
    #[snafu(display("query size {query_size} exceeds limit {QUERY_SIZE_LIMIT}"))]
    QuerySizeLimit {
        /// The actual query size.
        query_size: usize,
    },
    /// Failed to parse query.
    #[snafu(display("failed to parse query: {source}"), context(false))]
    QueryParse {
        /// The source parser error.
        source: sqlparser::parser::ParserError,
    },
    /// Expected exactly one sql statement in query input.
    #[snafu(display("expected exactly one sql statement in query input, found {num_statements}"))]
    NotOneStatement {
        /// The actual number of statements
        num_statements: usize,
    },
    /// Encountered proof-of-sql incompatible relation.
    #[snafu(
        display("encountered proof-of-sql incompatible relation: {source}"),
        context(false)
    )]
    ProofOfSqlIncompatibleRelation {
        /// The source proof-of-sql error.
        source: proof_of_sql::base::database::ParseError,
    },
    /// Received error from runtime api.
    #[snafu(display("received error from runtime api: {source}"), context(false))]
    RuntimeApi {
        /// The source runtime api error.
        source: sp_api::ApiError,
    },
    /// Unexpected table to commitment mismap.
    #[snafu(display("unexpected table ref to commitment mismap, statement has {num_tables} tables but runtime api returned {num_commitments} commitments"))]
    UnexpectedTableCommitmentMismap {
        num_tables: usize,
        num_commitments: usize,
    },
    /// Failed to deserialize table commitment.
    #[snafu(display("failed to deserialize table commitment: {source}"))]
    DeserializeTableCommitment {
        /// The source bincode error.
        source: bincode::error::DecodeError,
    },
    /// Encountered error in proof-of-sql planner.
    #[snafu(
        display("encountered error in proof-of-sql planner: {source}"),
        context(false)
    )]
    Planner {
        /// The osource planner error.
        source: PlannerError,
    },
    /// Tables do not exist or have incomplete commitment coverage for all schemes.
    #[snafu(display(
        "tables do not exist or have incomplete commitment coverage for all schemes"
    ))]
    IncompleteCommitmentCoverage,
    /// Failed to encode proof plan.
    #[snafu(display("failed to encode proof plan: {source}"), context(false))]
    EncodeProofPlan {
        /// The source bincode error.
        source: bincode::error::EncodeError,
    },
}

impl From<CommitmentsApiError> for ErrorObjectOwned {
    fn from(error: CommitmentsApiError) -> Self {
        let message = error.to_string();
        let code = BASE_ERROR
            + match error {
                CommitmentsApiError::ProofPlanSizeLimit { .. } => 0,
                CommitmentsApiError::DeserializeProofPlan { .. } => 1,
                CommitmentsApiError::TableIdentifierConversion { .. } => 2,
                CommitmentsApiError::NumTablesLimit { .. } => 3,
                CommitmentsApiError::Storage { .. } => 4,
                CommitmentsApiError::NoSuchCommitment { .. } => 5,
                CommitmentsApiError::CommitmentDecode { .. } => 6,
                CommitmentsApiError::AttestationTree { .. } => 7,
                CommitmentsApiError::AttestationTreeProof { .. } => 8,
                CommitmentsApiError::NoStakingContract => 9,
                CommitmentsApiError::QuerySizeLimit { .. } => 10,
                CommitmentsApiError::QueryParse { .. } => 11,
                CommitmentsApiError::NotOneStatement { .. } => 12,
                CommitmentsApiError::ProofOfSqlIncompatibleRelation { .. } => 13,
                CommitmentsApiError::RuntimeApi { .. } => 14,
                CommitmentsApiError::UnexpectedTableCommitmentMismap { .. } => 15,
                CommitmentsApiError::DeserializeTableCommitment { .. } => 16,
                CommitmentsApiError::Planner { .. } => 17,
                CommitmentsApiError::IncompleteCommitmentCoverage => 18,
                CommitmentsApiError::EncodeProofPlan { .. } => 19,
            };

        ErrorObjectOwned::owned(code, message, None::<()>)
    }
}
