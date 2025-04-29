use indexmap::IndexMap;
use jsonrpsee::proc_macros::rpc;
use proof_of_sql_commitment_map::CommitmentScheme;
use serde::Serialize;
use sp_core::Bytes;

use crate::commitments::error::CommitmentsApiError;

/// Serialization format for a Commitment and its attestation merkle proof.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiableCommitment {
    /// The commitment bytes.
    pub commitment: Bytes,
    /// The merkle proof.
    ///
    /// The Strings here are always hex encoded bytes.
    pub merkle_proof: Vec<String>,
}

/// Serialization format for an api response returning verifiable commitments.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiableCommitmentsResponse<BH: Serialize> {
    /// The verifiable commitments.
    pub verifiable_commitments: IndexMap<String, VerifiableCommitment>,
    /// The block hash that this query accessed storage with.
    pub at: BH,
}

#[rpc(server)]
pub trait CommitmentsApi<BH: Serialize> {
    /// Returns commitments + their merkle proofs for all tables in the proof-of-sql proof plan.
    #[method(name = "commitments_v1_verifiableCommitmentsForProofPlan", blocking)]
    fn v1_verifiable_commitments_for_proof_plan(
        &self,
        proof_plan: Bytes,
        commitment_scheme: CommitmentScheme,
        at: Option<BH>,
    ) -> Result<VerifiableCommitmentsResponse<BH>, CommitmentsApiError>;
}
