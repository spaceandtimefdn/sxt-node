use core::fmt::Debug;

use commitment_sql::{
    AppendOnChainTableError,
    InvalidColumnOptions,
    InvalidCreateTable,
    ProcessCreateTableFromSnapshotError,
    ProcessInsertError,
    UnsupportedColumnType,
};
use proof_of_sql_commitment_map::{KeyExistsError, TableCommitmentToBytesError};

use crate::pallet::Error;

impl<T> From<TableCommitmentToBytesError> for Error<T> {
    fn from(error: TableCommitmentToBytesError) -> Self {
        match error {
            TableCommitmentToBytesError::TooManyColumns { .. } => {
                Error::CommitmentWithTooManyColumns
            }
            TableCommitmentToBytesError::Postcard { .. } => Error::SerializeCommitment,
        }
    }
}

impl<T> From<InvalidColumnOptions> for Error<T> {
    fn from(error: InvalidColumnOptions) -> Self {
        match error {
            InvalidColumnOptions::Required { .. } => Error::ColumnWithoutNotNull,
            InvalidColumnOptions::Unsupported { .. } => Error::ColumnWithUnsupportedOption,
        }
    }
}

impl<T> From<UnsupportedColumnType> for Error<T> {
    fn from(error: UnsupportedColumnType) -> Self {
        match error {
            UnsupportedColumnType::TimestampPrecision { .. } => {
                Error::TimestampColumnWithInvalidPrecision
            }
            UnsupportedColumnType::UnconstrainedDecimal => Error::DecimalColumnWithoutPrecision,
            UnsupportedColumnType::DecimalPrecision { .. } => {
                Error::DecimalColumnWithInvalidPrecision
            }
            UnsupportedColumnType::DecimalScale { .. } => Error::DecimalColumnWithInvalidScale,
            UnsupportedColumnType::DataType { .. } => Error::ColumnWithUnsupportedDataType,
        }
    }
}

impl<T> From<InvalidCreateTable> for Error<T> {
    fn from(error: InvalidCreateTable) -> Self {
        match error {
            InvalidCreateTable::NoColumns => Error::CreateTableWithNoColumns,
            InvalidCreateTable::UnsupportedColumnType { source } => source.into(),
            InvalidCreateTable::Identifier { .. } => Error::CreateTableWithInvalidIdentifier,
            InvalidCreateTable::DuplicateIdentifiers => Error::CreateTableWithDuplicateIdentifiers,
            InvalidCreateTable::ReservedMetadataPrefix { .. } => {
                Error::CreateTableWithReservedMetadataPrefix
            }
            InvalidCreateTable::ColumnOptions { source } => source.into(),
        }
    }
}

impl<T> From<ProcessCreateTableFromSnapshotError> for Error<T> {
    fn from(error: ProcessCreateTableFromSnapshotError) -> Self {
        match error {
            ProcessCreateTableFromSnapshotError::InvalidCreateTable { source } => source.into(),
            ProcessCreateTableFromSnapshotError::InappropriateSnapshotCommitments { .. } => {
                Error::InappropriateSnapshotCommitments
            }
        }
    }
}

impl<T> From<AppendOnChainTableError> for Error<T> {
    fn from(error: AppendOnChainTableError) -> Self {
        match error {
            AppendOnChainTableError::OutOfScalarBounds { .. } => Error::InsertDataOutOfBounds,
            AppendOnChainTableError::ColumnCommitmentsMismatch { .. } => {
                Error::InsertDataDoesntMatchExistingCommitments
            }
        }
    }
}

impl<T> From<ProcessInsertError> for Error<T> {
    fn from(error: ProcessInsertError) -> Self {
        match error {
            ProcessInsertError::AppendOnChainTable { source } => source.into(),
            ProcessInsertError::TableCommitmentRangeMismatch => {
                Error::ExistingCommitmentsRangeMismatch
            }
            ProcessInsertError::NoCommitments => Error::NoExistingCommitments,
        }
    }
}

impl<T, K: Debug> From<KeyExistsError<K>> for Error<T> {
    fn from(_: KeyExistsError<K>) -> Self {
        Error::TableAlreadyExists
    }
}
