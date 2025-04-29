use sxt_core::native::NativeError;

use crate::Error;

impl<T, I> From<NativeError> for Error<T, I> {
    fn from(error: NativeError) -> Self {
        match error {
            NativeError::DeserializationError => Error::NativeDeserializationError,
            NativeError::EmptyRecordBatchError => Error::NativeEmptyRecordBatchError,
            NativeError::BatchReadError => Error::NativeBatchReadError,
            NativeError::RecordBatchUnsupportedType => Error::NativeRecordBatchUnsupportedType,
            NativeError::RecordBatchContainsNulls => Error::NativeRecordBatchContainsNulls,
            NativeError::RecordBatchInvalidTimezone => Error::NativeRecordBatchInvalidTimezone,
            NativeError::RecordBatchUnexpectedSchemaDataMismatch => {
                Error::NativeRecordBatchUnexpectedSchemaDataMismatch
            }
            NativeError::RecordBatchDuplicateIdentifiers => {
                Error::NativeRecordBatchDuplicateIdentifiers
            }
            NativeError::SerializationError => Error::NativeSerializationError,
        }
    }
}
