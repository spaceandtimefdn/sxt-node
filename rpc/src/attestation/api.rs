use jsonrpsee::proc_macros::rpc;
use serde::Serialize;
use sxt_core::attestation::Attestation;

use crate::attestation::AttestationApiError;

/// Response containing attestation info used by the attestation RPCs.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttestationsResponse<BH: Serialize> {
    /// The attestations for the `attestations_for` block.
    pub attestations: Vec<Attestation<BH>>,
    /// The block hash that was attested.
    pub attestations_for: BH,
    /// The block number that was attested.
    pub attestations_for_block_number: u32,
    /// The block that was used to query storage.
    pub at: BH,
}

/// RPCs related to the attestation pallet.
#[rpc(server)]
pub trait AttestationApi<BH: Serialize> {
    /// Get all attestations for the provided block.
    #[method(name = "attestation_v1_attestationsForBlock")]
    fn v1_attestations_for_block(
        &self,
        attestations_for: BH,
        at: Option<BH>,
    ) -> Result<AttestationsResponse<BH>, AttestationApiError>;

    /// For all blocks in the last minute, gets the attestations for...
    /// 1. the block that has the most attestations
    /// 2. attestation count being equal, the block that is the most recent
    #[method(name = "attestation_v1_bestRecentAttestations")]
    fn v1_best_recent_attestations(
        &self,
        at: Option<BH>,
    ) -> Result<AttestationsResponse<BH>, AttestationApiError>;
}
