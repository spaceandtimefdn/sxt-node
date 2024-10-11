use crate::pallet::Error;
use commitment_sql::{
    InvalidColumnOptions, InvalidCreateTable, ProcessCreateTableFromSnapshotError,
    UnsupportedColumnType,
};
use core::fmt::Debug;
use proof_of_sql_commitment_map::{KeyExistsError, TableCommitmentToBytesError};

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
            UnsupportedColumnType::TimestampWithoutTimezone => {
                Error::TimestampColumnWithoutTimezone
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

impl<T, K: Debug> From<KeyExistsError<K>> for Error<T> {
    fn from(_: KeyExistsError<K>) -> Self {
        Error::TableAlreadyExists
    }
}
