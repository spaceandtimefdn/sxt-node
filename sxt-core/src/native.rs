use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_runtime_interface::pass_by::PassByCodec;

use crate::indexing;

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
    pub data: indexing::RowData,
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

    /// Error converting to an OnChainTable from a record batch
    OnChainTableConversionError,

    /// Error serializing the OnChainTable
    SerializationError,

    /// Error creating a bounded vector for this OnChainTable
    BoundedVecError,
}
