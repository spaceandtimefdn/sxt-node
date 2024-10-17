use alloc::vec::Vec;

use arrow::array::RecordBatch;
use proof_of_sql_parser::ParseError;
use snafu::Snafu;

use crate::map::IndexSet;
use crate::{ArrowToOnChainColumnError, OnChainColumn, OnChainTable};

/// Errors that can occur when converting a `RecordBatch` to an [`OnChainTable`].
#[derive(Debug, Snafu)]
pub enum ArrowToOnChainTableError {
    /// Failed to convert arrow `ArrayRef` to [`OnChainColumn`].
    #[snafu(display("failed to convert arrow column to on-chain type: {error}"))]
    Column {
        /// Error encountered by column conversion
        error: ArrowToOnChainColumnError,
    },
    /// Failed to parse column identifier.
    #[snafu(display("failed to parse column identifier: {error}"))]
    Identifier {
        /// Error encountered by parser
        error: ParseError,
    },
    /// Failed to parse column identifier.
    #[snafu(display("encountered duplicate column identifier in RecordBatch"))]
    DuplicateIdentifier,
}

impl From<ArrowToOnChainColumnError> for ArrowToOnChainTableError {
    fn from(error: ArrowToOnChainColumnError) -> Self {
        ArrowToOnChainTableError::Column { error }
    }
}

impl From<ParseError> for ArrowToOnChainTableError {
    fn from(error: ParseError) -> Self {
        ArrowToOnChainTableError::Identifier { error }
    }
}

impl TryFrom<RecordBatch> for OnChainTable {
    type Error = ArrowToOnChainTableError;
    fn try_from(batch: RecordBatch) -> Result<Self, Self::Error> {
        let columns = batch
            .columns()
            .iter()
            .map(OnChainColumn::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let identifiers = batch
            .schema()
            .fields()
            .into_iter()
            .map(|field| field.name().parse())
            .collect::<Result<IndexSet<_>, _>>()?;

        if columns.len() != identifiers.len() {
            return Err(ArrowToOnChainTableError::DuplicateIdentifier);
        }

        Ok(OnChainTable::try_from_iter(identifiers.into_iter().zip(columns)
        ).expect("RecordBatch guarantees that table has at least one column, and that columns have matching lengths"))
    }
}

impl From<OnChainTable> for RecordBatch {
    fn from(value: OnChainTable) -> Self {
        RecordBatch::try_from_iter(
            value
                .into_iter()
                .map(|(identifier, column)| (identifier, column.into())),
        )
        .expect("OnChainTable type guarantees all expectations of RecordBatches")
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use alloc::vec;

    use arrow::array::{ArrayRef, Float32Array, Int64Array, StringArray};

    use super::*;

    #[test]
    fn we_can_convert_table_to_and_from_record_batch() {
        let bigint_col_id = "bigint_col".parse().unwrap();
        let bigint_col_data = vec![1, 2, 3];
        let bigint_col_array: ArrayRef = Arc::new(Int64Array::from(bigint_col_data.clone()));
        let bigint_col_column = OnChainColumn::BigInt(bigint_col_data);

        let varchar_col_id = "varchar_col".parse().unwrap();
        let varchar_col_data = ["lorem", "ipsum", "dolor"].map(String::from).to_vec();
        let varchar_col_array: ArrayRef = Arc::new(StringArray::from(varchar_col_data.clone()));
        let varchar_col_column = OnChainColumn::VarChar(varchar_col_data);

        let record_batch = RecordBatch::try_from_iter([
            (bigint_col_id, bigint_col_array),
            (varchar_col_id, varchar_col_array),
        ])
        .unwrap();
        let table = OnChainTable::try_from_iter([
            (bigint_col_id, bigint_col_column),
            (varchar_col_id, varchar_col_column),
        ])
        .unwrap();

        assert_eq!(OnChainTable::try_from(record_batch.clone()).unwrap(), table);
        assert_eq!(RecordBatch::from(table), record_batch);
    }

    #[test]
    fn we_cannot_convert_table_from_batch_with_unsupported_column() {
        let float_col_id = "float_col";
        let float_col_data = vec![1., 2., 3.];
        let float_col_array: ArrayRef = Arc::new(Float32Array::from(float_col_data.clone()));

        let record_batch = RecordBatch::try_from_iter([(float_col_id, float_col_array)]).unwrap();
        assert!(matches!(
            OnChainTable::try_from(record_batch),
            Err(ArrowToOnChainTableError::Column { .. })
        ));
    }

    #[test]
    fn we_cannot_convert_table_from_batch_with_malformed_identifier() {
        let bigint_col_id = "bad.id";
        let bigint_col_data = vec![1, 2, 3];
        let bigint_col_array: ArrayRef = Arc::new(Int64Array::from(bigint_col_data.clone()));

        let record_batch = RecordBatch::try_from_iter([(bigint_col_id, bigint_col_array)]).unwrap();
        assert!(matches!(
            OnChainTable::try_from(record_batch),
            Err(ArrowToOnChainTableError::Identifier { .. })
        ));
    }

    #[test]
    fn we_cannot_convert_table_from_batch_with_duplicate_identifiers() {
        let duplicate_id = "duplicate_id";

        let bigint_col_data = vec![1, 2, 3];
        let bigint_col_array: ArrayRef = Arc::new(Int64Array::from(bigint_col_data.clone()));

        let varchar_col_data = ["lorem", "ipsum", "dolor"].map(String::from).to_vec();
        let varchar_col_array: ArrayRef = Arc::new(StringArray::from(varchar_col_data.clone()));

        let record_batch = RecordBatch::try_from_iter([
            (duplicate_id, bigint_col_array),
            (duplicate_id, varchar_col_array),
        ])
        .unwrap();
        assert!(matches!(
            OnChainTable::try_from(record_batch),
            Err(ArrowToOnChainTableError::DuplicateIdentifier)
        ));
    }
}
