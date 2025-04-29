use codec::{Decode, Encode, MaxEncodedLen};
use on_chain_table::OnChainTable;
#[cfg(feature = "std")]
use on_chain_table::{ArrowToOnChainColumnError, ArrowToOnChainTableError};
use scale_info::TypeInfo;
use snafu::Snafu;
use sp_core::RuntimeDebug;
use sp_runtime_interface::pass_by::PassByCodec;

use crate::indexing;
use crate::tables::CreateStatement;

/// Wrapper around [`CreateStatement`], needed to pass to pass the WASM boundary easily.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, PassByCodec)]
pub struct CreateStatementPassBy {
    /// A create statement represented as a string, of bytes.
    pub create_statement: CreateStatement,
}

/// Wrapper around sxt_core::indexing::RowData, needed to pass the WASM boundary easily
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, PassByCodec)]
pub struct RowData {
    /// An arrow record batch represented as bytes in IPC format
    pub row_data: indexing::RowData,
}

/// A wrapper for the return type of the native method to convert row_data into a serialized OnChainTable
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, PassByCodec)]
pub struct OnChainTableBytes {
    /// A serialized OnChainTable represented as bytes
    data: indexing::RowData,
}

impl OnChainTableBytes {
    /// Returns the underlying bytes.
    pub fn data(&self) -> &indexing::RowData {
        &self.data
    }
}

impl TryFrom<OnChainTableBytes> for OnChainTable {
    type Error = postcard::Error;

    fn try_from(value: OnChainTableBytes) -> Result<Self, Self::Error> {
        postcard::from_bytes(value.data())
    }
}

/// Errors that can occur when encoding an `OnChainTable` to bytes.
#[derive(Debug, Snafu)]
pub enum OnChainTableToBytesError {
    ///  Unable to serialize `OnChainTable`.
    #[snafu(display("unable to serialize OnChainTable: {error}"))]
    Serialize {
        /// The source postcard error.
        error: postcard::Error,
    },
    /// `OnChainTable` exceeds the maximum size for this byte encoding.
    #[snafu(display("OnChainTable exceeds the maximum size for this byte encoding"))]
    ExceedsMaxSize,
}

impl TryFrom<OnChainTable> for OnChainTableBytes {
    type Error = OnChainTableToBytesError;

    fn try_from(value: OnChainTable) -> Result<Self, Self::Error> {
        let bytes = postcard::to_allocvec(&value)
            .map_err(|error| OnChainTableToBytesError::Serialize { error })?;

        let data = bytes
            .try_into()
            .map_err(|_| OnChainTableToBytesError::ExceedsMaxSize)?;

        Ok(OnChainTableBytes { data })
    }
}

/// Errors that can occur in the native code interface
#[derive(Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum NativeError {
    /// The table could not be deserialized using a Stream Reader
    DeserializationError,

    /// There was no record batch contained in the data
    EmptyRecordBatchError,

    /// Error reading record batch
    BatchReadError,

    /// RecordBatch column has unsupported type
    RecordBatchUnsupportedType,

    /// RecordBatch contains nulls
    RecordBatchContainsNulls,

    /// RecordBatch has invalid timezone
    RecordBatchInvalidTimezone,

    /// RecordBatch has unexpected mismatch between schema and data
    RecordBatchUnexpectedSchemaDataMismatch,

    /// RecordBatch has duplicate identifiers
    RecordBatchDuplicateIdentifiers,

    /// Error serializing the OnChainTable
    SerializationError,
}

impl From<OnChainTableToBytesError> for NativeError {
    fn from(_: OnChainTableToBytesError) -> Self {
        NativeError::SerializationError
    }
}

#[cfg(feature = "std")]
impl From<ArrowToOnChainColumnError> for NativeError {
    fn from(error: ArrowToOnChainColumnError) -> Self {
        match error {
            ArrowToOnChainColumnError::UnsupportedType { .. } => {
                NativeError::RecordBatchUnsupportedType
            }
            ArrowToOnChainColumnError::UnsupportedNull => NativeError::RecordBatchContainsNulls,
            ArrowToOnChainColumnError::InvalidTimestamp { .. } => {
                NativeError::RecordBatchInvalidTimezone
            }
            ArrowToOnChainColumnError::UnexpectedSchemaDataMismatch => {
                NativeError::RecordBatchUnexpectedSchemaDataMismatch
            }
        }
    }
}

#[cfg(feature = "std")]
impl From<ArrowToOnChainTableError> for NativeError {
    fn from(error: ArrowToOnChainTableError) -> Self {
        match error {
            ArrowToOnChainTableError::Column { error } => error.into(),
            ArrowToOnChainTableError::DuplicateIdentifier => {
                NativeError::RecordBatchDuplicateIdentifiers
            }
        }
    }
}

/// Errors that can occur in the native commitment computation functions.
#[derive(Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum NativeCommitmentError {
    /// The commitment failed to deserialize.
    CommitmentDeserialization,
    /// The table failed to deserialize
    TableDeserialization,
    /// Attempted to compute commitment to out of bounds data.
    OutOfScalarBounds,
    /// Commitment metadata indicates that operand tables cannot be the same.
    ColumnCommitmentsMismatch,
    /// Table commitments (of different schemes) have different ranges.
    TableCommitmentRangeMismatch,
    /// Table commitments (of different schemes) have different column orders.
    TableCommitmentColumnOrderMismatch,
    /// No commitments to update.
    NoCommitments,
    /// The commitment failed to serialize.
    CommitmentSerialization,
    /// The table failed to serialize
    TableSerialization,
}

impl From<OnChainTableToBytesError> for NativeCommitmentError {
    fn from(_: OnChainTableToBytesError) -> Self {
        NativeCommitmentError::TableSerialization
    }
}
