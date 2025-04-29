use attestation_tree::{AttestationTreeError, AttestationTreeProofError};
use jsonrpsee::types::ErrorObjectOwned;
use snafu::Snafu;
use sxt_core::tables::TableIdentifierConversionError;

use crate::commitments::limits::{NUM_TABLES_LIMIT, PROOF_PLAN_SIZE_LIMIT};

/// The base error code used by the commitments RPCs.
const BASE_ERROR: i32 = 254000;

/// Errors that can occur in the commitments RPCs.
#[derive(Debug, Snafu)]
pub enum CommitmentsApiError {
    /// Proof plan size exceeds limit.
    #[snafu(display(
        "proof plan of size {proof_plan_size} exceeds limit {PROOF_PLAN_SIZE_LIMIT}"
    ))]
    ProofPlanSizeLimit { proof_plan_size: usize },
    /// Failed to deserialize proof plan.
    #[snafu(display("failed to deserialize proof plan: {source}"), context(false))]
    DeserializeProofPlan { source: bincode::error::DecodeError },
    /// Failed to convert table identifier from proof-of-sql to sxt-chain type.
    #[snafu(
        display(
            "failed to convert table identifier from proof-of-sql to sxt-chain type: {source}"
        ),
        context(false)
    )]
    TableIdentifierConversion {
        source: TableIdentifierConversionError,
    },
    /// Number of tables exceeds limit.
    #[snafu(display("query against {num_tables} tables exceeds limit of {NUM_TABLES_LIMIT}"))]
    NumTablesLimit { num_tables: usize },
    /// Failed to query storage.
    #[snafu(display("failed to query storage: {source}"), context(false))]
    Storage { source: sp_blockchain::Error },
    /// Commitment does not exist in storage.
    #[snafu(display("commitment does not exist in storage"))]
    NoSuchCommitment,
    /// Failed to decode commitment in storage.
    #[snafu(display("failed to decode commitment in storage"), context(false))]
    CommitmentDecode { source: codec::Error },
    /// Failed to create attestation tree.
    #[snafu(display("failed to create attestation tree: {source}"), context(false))]
    AttestationTree { source: AttestationTreeError },
    /// Failed to generate merkle proof for commitments.
    #[snafu(
        display("failed to generate merkle proof for commitment: {source}"),
        context(false)
    )]
    AttestationTreeProof { source: AttestationTreeProofError },
    /// Staking contract info is not defined in storage.
    #[snafu(display("staking contract info is not defined in storage"))]
    NoStakingContract,
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
            };

        ErrorObjectOwned::owned(code, message, None::<()>)
    }
}
