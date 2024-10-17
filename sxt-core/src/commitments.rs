use alloc::vec::Vec;

use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Compress, Validate};
use codec::{Decode, Encode, MaxEncodedLen};
use proof_of_sql::proof_primitive::dory::PublicParameters;
use scale_info::TypeInfo;
use snafu::Snafu;
use sp_core::ConstU32;
use sp_runtime::BoundedVec;

/// Maximum size of proof-of-sql public parameters as a constant.
const PUBLIC_PARAMETERS_MAX_SIZE: u32 = 290_000_000;

/// Maximum size of proof-of-sql public parameters as a type.
type PublicParametersMaxSize = ConstU32<PUBLIC_PARAMETERS_MAX_SIZE>;

/// Proof-of-sql public parameters serialized as bytes.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct PublicParametersBytes {
    /// PublicParameters serialized with ark-serialize.
    pub data: BoundedVec<u8, PublicParametersMaxSize>,
}

// NOTE: this conversion is difficult to test because PublicParameters..
// - PublicParameters is expensive to generate
// - PublicParameters does not implement Debug or Clone or Eq
impl TryFrom<PublicParametersBytes> for PublicParameters {
    type Error = ark_serialize::SerializationError;

    fn try_from(value: PublicParametersBytes) -> Result<Self, Self::Error> {
        PublicParameters::deserialize_with_mode(value.data.as_slice(), Compress::No, Validate::No)
    }
}

/// Errors that can occur when converting `PublicParameters` to [`PublicParametersBytes`].
#[derive(Debug, Snafu)]
pub enum PublicParametersToBytesError {
    /// Failed to serialize `PublicParameters`.
    #[snafu(display("Failed to serialize PublicParameters"))]
    Serialize {
        /// The source serialization error.
        error: ark_serialize::SerializationError,
    },
    /// Serialized PublicParameters bytes exceed max size.
    #[snafu(display("Serialized PublicParameters bytes exceed max size"))]
    BytesExceedMaxLength,
}

// NOTE: this conversion is difficult to test because PublicParameters..
// - PublicParameters is expensive to generate
// - PublicParameters does not implement Debug or Clone or Eq
impl From<ark_serialize::SerializationError> for PublicParametersToBytesError {
    fn from(error: ark_serialize::SerializationError) -> Self {
        PublicParametersToBytesError::Serialize { error }
    }
}

impl TryFrom<PublicParameters> for PublicParametersBytes {
    type Error = PublicParametersToBytesError;

    fn try_from(value: PublicParameters) -> Result<Self, Self::Error> {
        let mut buffer = Vec::new();
        value.serialize_with_mode(&mut buffer, Compress::No)?;

        let data = BoundedVec::try_from(buffer)
            .map_err(|_| PublicParametersToBytesError::BytesExceedMaxLength)?;

        Ok(PublicParametersBytes { data })
    }
}
