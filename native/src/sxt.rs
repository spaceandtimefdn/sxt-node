//! The native code implementation
#[cfg(feature = "std")]
use arrow::ipc::reader::StreamReader;
#[cfg(feature = "std")]
use commitment_sql::InsertAndCommitmentMetadata;
use proof_of_sql_commitment_map::{
    PerCommitmentScheme,
    TableCommitmentBytesPerCommitmentScheme,
    TableCommitmentBytesPerCommitmentSchemePassBy,
};
#[cfg(feature = "std")]
use proof_of_sql_static_setups::io::PUBLIC_SETUPS;
use sp_runtime_interface::runtime_interface;
use sxt_core::native::{
    CreateStatementPassBy,
    NativeCommitmentError,
    NativeError,
    OnChainTableBytes,
    RowData,
};
use sxt_core::tables::TableIdentifier;

/// Space and Time's native code interface
#[runtime_interface]
pub trait Interface {
    /// Convert a sxt_core::native::RowData into a serialized OnChainTable.
    /// RowData is a wrapper around a bounded vec that contains the table in IPC format.
    /// After the table is parsed into a record batch we convert it into an OnChainTable and then serialize it to pass back into the runtime.
    fn record_batch_to_onchain(row_data: RowData) -> Result<OnChainTableBytes, NativeError> {
        let mut reader = StreamReader::try_new(row_data.row_data.as_slice(), None)
            .map_err(|_| NativeError::DeserializationError)?;

        let batch = reader
            .next()
            .ok_or(NativeError::EmptyRecordBatchError)?
            .map_err(|_| NativeError::BatchReadError)?;

        let on_chain_table = on_chain_table::OnChainTable::try_from(batch)?;

        Ok(OnChainTableBytes::try_from(on_chain_table)?)
    }

    /// Convert a sxt_core::native::RowData into a serialized OnChainTable, and force data
    /// compliance in accordance with the table's create statement.
    ///
    /// RowData is a wrapper around a bounded vec that contains the table in IPC format.
    /// After the table is parsed into a record batch, we apply data-compliance functions, then we
    /// convert it into an OnChainTable and then serialize it to pass back into the runtime.
    #[version(2)]
    fn record_batch_to_onchain(
        row_data: RowData,
        _create_statement: CreateStatementPassBy,
    ) -> Result<OnChainTableBytes, NativeError> {
        let mut reader = StreamReader::try_new(row_data.row_data.as_slice(), None)
            .map_err(|_| NativeError::DeserializationError)?;

        let batch = reader
            .next()
            .ok_or(NativeError::EmptyRecordBatchError)?
            .map_err(|_| NativeError::BatchReadError)?;

        let on_chain_table = on_chain_table::OnChainTable::try_from(batch)?;

        Ok(OnChainTableBytes::try_from(on_chain_table)?)
    }

    /// Convert a sxt_core::native::RowData into a serialized OnChainTable.
    /// RowData is a wrapper around a bounded vec that contains the table in IPC format.
    /// After the table is parsed into a record batch we convert it into an OnChainTable and then serialize it to pass back into the runtime.
    #[version(3)]
    fn record_batch_to_onchain(row_data: RowData) -> Result<OnChainTableBytes, NativeError> {
        let mut reader = StreamReader::try_new(row_data.row_data.as_slice(), None)
            .map_err(|_| NativeError::DeserializationError)?;

        let batch = reader
            .next()
            .ok_or(NativeError::EmptyRecordBatchError)?
            .map_err(|_| NativeError::BatchReadError)?;

        let on_chain_table = on_chain_table::OnChainTable::try_from(batch)?;

        Ok(OnChainTableBytes::try_from(on_chain_table)?)
    }

    /// Process insert to support commitment metadata.
    ///
    /// Returns..
    /// - the processed insert data with comitment metadata
    /// - the updated commitments for the table
    fn process_insert(
        table_identifier: TableIdentifier,
        insert_data_bytes: OnChainTableBytes,
        previous_commitments_bytes: TableCommitmentBytesPerCommitmentSchemePassBy,
    ) -> Result<
        (
            OnChainTableBytes,
            TableCommitmentBytesPerCommitmentSchemePassBy,
        ),
        NativeCommitmentError,
    > {
        let insert_data = on_chain_table::OnChainTable::try_from(insert_data_bytes)
            .map_err(|_| NativeCommitmentError::TableDeserialization)?;

        let previous_commitments = PerCommitmentScheme::try_from(previous_commitments_bytes.data)
            .map_err(|_| NativeCommitmentError::CommitmentDeserialization)?;

        let setups = PUBLIC_SETUPS
            .get()
            .expect("PUBLIC_SETUPS should be initialized before runtime interface calls");

        let (
            InsertAndCommitmentMetadata {
                insert_with_meta_columns,
                ..
            },
            new_commitments,
        ) = commitment_sql::process_insert(
            &table_identifier,
            insert_data,
            previous_commitments,
            *setups,
        )?;

        let table_bytes = insert_with_meta_columns.try_into()?;

        let data = TableCommitmentBytesPerCommitmentScheme::try_from(new_commitments)?;

        let new_commitments_bytes = TableCommitmentBytesPerCommitmentSchemePassBy { data };

        Ok((table_bytes, new_commitments_bytes))
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::io::Cursor;
    use std::sync::Arc;

    use arrow::array::{ArrayRef, Int32Array, RecordBatch, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::ipc::writer::StreamWriter;
    use commitment_sql::OnChainTableToTableCommitmentFn;
    use on_chain_table::{OnChainColumn, OnChainTable};
    use proof_of_sql::base::database::ColumnType;
    use proof_of_sql::base::math::decimal::Precision;
    use proof_of_sql_commitment_map::generic_over_commitment::{OptionType, TableCommitmentType};
    use proof_of_sql_commitment_map::TableCommitmentBytes;
    use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
    use sp_core::U256;
    use sp_runtime::BoundedVec;
    use sqlparser::ast::Ident;
    use sxt_core::tables::create_statement;

    use super::*;

    fn row_data() -> RowData {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "int_column",
            DataType::Int32,
            false,
        )]));

        let int_data = Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5])) as ArrayRef;

        let batch = RecordBatch::try_new(schema.clone(), vec![int_data]).unwrap();

        let buffer: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(buffer);

        let mut writer = StreamWriter::try_new(&mut cursor, &schema).unwrap();

        writer.write(&batch).unwrap();
        writer.finish().unwrap();

        let data = writer.into_inner().unwrap().clone();
        let data = data.into_inner().clone();

        RowData {
            row_data: BoundedVec::try_from(data).unwrap(),
        }
    }

    #[test]
    fn conversion_works() {
        let res = interface::record_batch_to_onchain(row_data());
        assert!(res.is_ok());
    }

    fn sample_empty_and_populated_on_chain_table() -> (OnChainTable, OnChainTable) {
        let animals_col_id = Ident::new("animals");
        let animals_data = ["cow", "dog", "cat"].map(String::from);

        let population_col_id = Ident::new("population");
        let population_data = [100, 2, 7];

        let empty_table = OnChainTable::try_from_iter([
            (
                animals_col_id.clone(),
                OnChainColumn::empty_with_type(ColumnType::VarChar),
            ),
            (
                population_col_id.clone(),
                OnChainColumn::empty_with_type(ColumnType::BigInt),
            ),
        ])
        .unwrap();

        let populated_table = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::VarChar(animals_data.to_vec()),
            ),
            (
                population_col_id,
                OnChainColumn::BigInt(population_data.to_vec()),
            ),
        ])
        .unwrap();

        (empty_table, populated_table)
    }

    #[test]
    fn we_can_process_inserts() {
        let setups = get_or_init_from_files_with_four_points_unchecked();
        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let (empty_table, insert_data) = sample_empty_and_populated_on_chain_table();
        let insert_data_bytes = OnChainTableBytes::try_from(insert_data.clone()).unwrap();

        let empty_commitments = setups
            .into_iter()
            .map(|any| {
                any.map(OnChainTableToTableCommitmentFn::new(&empty_table, 0))
                    .transpose_result()
                    .unwrap()
            })
            .collect::<PerCommitmentScheme<OptionType<TableCommitmentType>>>();

        let empty_commitments_bytes = TableCommitmentBytesPerCommitmentSchemePassBy {
            data: empty_commitments.clone().try_into().unwrap(),
        };

        let (insert_with_meta_columns, new_commitments) =
            interface::process_insert(table_id.clone(), insert_data_bytes, empty_commitments_bytes)
                .unwrap();

        let (
            InsertAndCommitmentMetadata {
                insert_with_meta_columns: expected_insert_with_meta_columns,
                ..
            },
            expected_commitments,
        ) = commitment_sql::process_insert(&table_id, insert_data, empty_commitments, *setups)
            .unwrap();

        assert_eq!(
            insert_with_meta_columns,
            expected_insert_with_meta_columns.try_into().unwrap()
        );
        assert_eq!(
            new_commitments.data,
            expected_commitments.try_into().unwrap()
        );
    }

    #[test]
    fn we_cannot_process_insert_with_invalid_commitment_bytes() {
        let _ = get_or_init_from_files_with_four_points_unchecked();

        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let (_, insert_data) = sample_empty_and_populated_on_chain_table();

        let insert_data_bytes = OnChainTableBytes::try_from(insert_data.clone()).unwrap();

        let invalid_commitments = TableCommitmentBytesPerCommitmentSchemePassBy {
            data: TableCommitmentBytesPerCommitmentScheme {
                hyper_kzg: None,
                dynamic_dory: Some(TableCommitmentBytes {
                    data: insert_data_bytes
                        .data()
                        .clone()
                        .into_inner()
                        .try_into()
                        .unwrap(),
                }),
            },
        };

        let result = interface::process_insert(table_id, insert_data_bytes, invalid_commitments);

        assert!(matches!(
            result,
            Err(NativeCommitmentError::CommitmentDeserialization)
        ));
    }

    #[test]
    fn we_cannot_process_insert_with_commitment_sql_failure() {
        let _ = get_or_init_from_files_with_four_points_unchecked();

        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let (_, insert_data) = sample_empty_and_populated_on_chain_table();

        let insert_data_bytes = OnChainTableBytes::try_from(insert_data.clone()).unwrap();

        let no_commitments = TableCommitmentBytesPerCommitmentSchemePassBy {
            data: TableCommitmentBytesPerCommitmentScheme::from_iter([]),
        };

        let result = interface::process_insert(table_id, insert_data_bytes, no_commitments);

        assert!(matches!(result, Err(NativeCommitmentError::NoCommitments)));
    }
}
