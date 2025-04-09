use jsonrpsee::types::ErrorObjectOwned;
use snafu::Snafu;

/// The base error code used by the attestation RPCs.
const BASE_ERROR: i32 = 255000;

/// Errors that can occur in the attestation RPCs
#[derive(Snafu, Debug)]
pub enum AttestationApiError {
    /// Failed to query the chain.
    #[snafu(display("failed to query the chain: {source}"), context(false))]
    ChainQuery { source: sp_blockchain::Error },
    /// Failed to get block number for provided hash.
    #[snafu(display("failed to get block number for provided hash"))]
    BlockNumberQuery,
    /// Failed to get block hash for provided number.
    #[snafu(display("failed to get block hash for provided number"))]
    BlockHashQuery,
    /// Failed to decode attestations.
    #[snafu(display("failed to decode attestations: {source}"), context(false))]
    DecodeAttestations { source: codec::Error },
}

impl From<AttestationApiError> for ErrorObjectOwned {
    fn from(error: AttestationApiError) -> Self {
        let message = error.to_string();

        let code = BASE_ERROR
            + match error {
                AttestationApiError::ChainQuery { .. } => 0,
                AttestationApiError::BlockNumberQuery => 1,
                AttestationApiError::BlockHashQuery => 2,
                AttestationApiError::DecodeAttestations { .. } => 3,
            };

        ErrorObjectOwned::owned(code, message, None::<()>)
    }
}
