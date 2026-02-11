use arrow_ipc_no_std::{
    ArrowRecordBatchParseError,
    ArrowSchemaParseError,
    FinishParseError,
    SingleBatchStreamParseError,
};
use sxt_core::native::NativeError;
use sxt_core::record_batch::RecordBatchBytesRowCountError;

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

impl<T, I> From<ArrowRecordBatchParseError> for Error<T, I> {
    fn from(error: ArrowRecordBatchParseError) -> Self {
        match error {
            ArrowRecordBatchParseError::ArrowMessageParse { .. } => {
                Error::ArrowParseRecordBatchMessage
            }
            ArrowRecordBatchParseError::ExpectedRecordBatch => {
                Error::ArrowExpectedRecordBatchMessage
            }
        }
    }
}

impl<T, I> From<ArrowSchemaParseError> for Error<T, I> {
    fn from(error: ArrowSchemaParseError) -> Self {
        match error {
            ArrowSchemaParseError::ArrowMessageParse { .. } => Error::ArrowParseSchemaMessage,
            ArrowSchemaParseError::ExpectedSchema => Error::ArrowExpectedSchemaMessage,
        }
    }
}

impl<T, I> From<SingleBatchStreamParseError> for Error<T, I> {
    fn from(error: SingleBatchStreamParseError) -> Self {
        match error {
            SingleBatchStreamParseError::Schema { source } => source.into(),
            SingleBatchStreamParseError::RecordBatch { source } => source.into(),
        }
    }
}

impl<T, I, E> From<FinishParseError<E>> for Error<T, I>
where
    E: Into<Error<T, I>> + snafu::AsErrorSource + core::fmt::Display,
{
    fn from(error: FinishParseError<E>) -> Self {
        match error {
            FinishParseError::Parse { source } => source.into(),
            FinishParseError::Unfinished => Error::ArrowParserUnfinished,
        }
    }
}

impl<T, I> From<RecordBatchBytesRowCountError> for Error<T, I> {
    fn from(error: RecordBatchBytesRowCountError) -> Self {
        match error {
            RecordBatchBytesRowCountError::Parse { source } => source.into(),
            RecordBatchBytesRowCountError::OutOfU32Bounds => Error::ArrowParserUnfinished,
        }
    }
}
