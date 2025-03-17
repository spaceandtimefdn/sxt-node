//! The native code implementation
#[cfg(feature = "std")]
use arrow::ipc::reader::StreamReader;
#[cfg(feature = "std")]
use commitment_sql::InsertAndCommitmentMetadata;
#[cfg(feature = "std")]
use data_compliance_please_deprecate_me::{
    column_clamp_precision,
    column_default_nulls,
    column_parse_decimals_fallible,
    column_remove_null_bytes,
    record_batch_map,
    record_batch_try_map_with_target_types,
    target_types_for_table,
};
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
use sxt_core::tables::{create_statement_to_sqlparser, TableIdentifier};

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

        let compliant_batch = record_batch_map(batch, |column| {
            column_clamp_precision(column_default_nulls(column_remove_null_bytes(column)))
        });

        let on_chain_table = on_chain_table::OnChainTable::try_from(compliant_batch)
            .map_err(|_| NativeError::OnChainTableConversionError)?;

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
        create_statement: CreateStatementPassBy,
    ) -> Result<OnChainTableBytes, NativeError> {
        let mut reader = StreamReader::try_new(row_data.row_data.as_slice(), None)
            .map_err(|_| NativeError::DeserializationError)?;

        let batch = reader
            .next()
            .ok_or(NativeError::EmptyRecordBatchError)?
            .map_err(|_| NativeError::BatchReadError)?;

        let create_table = create_statement_to_sqlparser(create_statement.create_statement)
            .map_err(|_| NativeError::DdlParseError)?;

        let target_types = target_types_for_table(create_table);

        let batch_with_parsed_decimals = record_batch_try_map_with_target_types(
            batch,
            &target_types,
            column_parse_decimals_fallible,
        )
        .map_err(|_| NativeError::DecimalParseError)?;

        let compliant_batch = record_batch_map(batch_with_parsed_decimals, |column| {
            column_clamp_precision(column_default_nulls(column_remove_null_bytes(column)))
        });

        let on_chain_table = on_chain_table::OnChainTable::try_from(compliant_batch)
            .map_err(|_| NativeError::OnChainTableConversionError)?;

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
    use on_chain_table::{OnChainColumn, OnChainTable};
    use proof_of_sql::base::commitment::TableCommitment;
    use proof_of_sql::base::database::ColumnType;
    use proof_of_sql::base::math::decimal::Precision;
    use proof_of_sql::proof_primitive::dory::{DoryScalar, DynamicDoryCommitment};
    use proof_of_sql_commitment_map::generic_over_commitment::{OptionType, TableCommitmentType};
    use proof_of_sql_commitment_map::TableCommitmentBytes;
    use proof_of_sql_static_setups::io::initialize_from_file_unchecked;
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
        let create_statement = create_statement("CREATE TABLE test.table (int_column Int)");

        let res = interface::record_batch_to_onchain(
            row_data(),
            CreateStatementPassBy { create_statement },
        );
        assert!(res.is_ok());
    }

    #[test]
    fn we_can_perform_data_compliance() {
        let id_column: ArrayRef = Arc::new(StringArray::from_iter([
            Some("lorem"),
            Some("ipsum"),
            None,
            Some("\0do\0lor"),
        ]));
        let price_column: ArrayRef = Arc::new(StringArray::from_iter([
            Some("0"),
            None,
            Some("-10e2"),
            Some("57896044618658097711785492504343953926634992332820282019728792003956564819967"),
        ]));
        let illegal_batch =
            RecordBatch::try_from_iter([("iD", id_column), ("pRIce", price_column)]).unwrap();

        let mut buffer: Vec<u8> = Vec::new();
        let mut writer = StreamWriter::try_new(&mut buffer, &illegal_batch.schema()).unwrap();
        writer.write(&illegal_batch).unwrap();
        writer.finish().unwrap();

        let illegal_batch_bytes = RowData {
            row_data: BoundedVec::try_from(buffer).unwrap(),
        };

        let create_statement = CreateStatementPassBy {
            create_statement: create_statement(
                "CREATE TABLE test.table (Id VARCHAR NULL, pricE DECIMAL(78, 0) NULL)",
            ),
        };

        let expected_on_chain_table = OnChainTable::try_from_iter([
            (
                Ident::new("iD"),
                OnChainColumn::VarChar(
                    ["lorem", "ipsum", "", "dolor"]
                        .map(ToString::to_string)
                        .to_vec(),
                ),
            ),
            (
                Ident::new("pRIce"),
                OnChainColumn::Decimal75(Precision::new(75).unwrap(), 0, vec![U256::zero(), U256::zero(), U256::MAX - U256::from(999), U256::from_str_radix("896044618658097711785492504343953926634992332820282019728792003956564819967", 10).unwrap()])
            ),
        ]).unwrap();

        let result: OnChainTable =
            interface::record_batch_to_onchain(illegal_batch_bytes, create_statement)
                .unwrap()
                .try_into()
                .unwrap();

        assert_eq!(result, expected_on_chain_table);
    }

    #[test]
    fn we_can_convert_ethereum_blocks_batch() {
        let batch_bytes = include_bytes!("../test-ethereum-blocks-batch");

        let row_data = RowData {
            row_data: BoundedVec::try_from(batch_bytes.to_vec()).unwrap(),
        };

        let create_statement = CreateStatementPassBy {
            create_statement: create_statement(
                "CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS(
              BLOCK_NUMBER BIGINT NOT NULL,
              TIME_STAMP TIMESTAMP,
              BLOCK_HASH VARCHAR,
              MINER VARCHAR,
              REWARD DECIMAL(78, 0),
              SIZE_ INT,
              GAS_USED INT,
              GAS_LIMIT INT,
              BASE_FEE_PER_GAS DECIMAL(78, 0),
              TRANSACTION_COUNT INT,
              PARENT_HASH VARCHAR,
              PRIMARY KEY(BLOCK_NUMBER)
            );",
            ),
        };

        let res = interface::record_batch_to_onchain(row_data, create_statement);
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
        let _ = initialize_from_file_unchecked(
            &"../proof-of-sql/static-setups/public_parameters_nu_15"
                .parse()
                .unwrap(),
        );
        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let (empty_table, insert_data) = sample_empty_and_populated_on_chain_table();
        let insert_data_bytes = OnChainTableBytes::try_from(insert_data.clone()).unwrap();

        let empty_commitments = PerCommitmentScheme::<OptionType<TableCommitmentType>> {
            ipa: None,
            dynamic_dory: Some(
                TableCommitment::<DynamicDoryCommitment>::try_from_columns_with_offset(
                    empty_table
                        .iter_committable::<DoryScalar>()
                        .map(Result::unwrap),
                    0,
                    &PUBLIC_SETUPS.get().unwrap().dynamic_dory,
                )
                .unwrap(),
            ),
        };

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
        ) = commitment_sql::process_insert(
            &table_id,
            insert_data,
            empty_commitments,
            *PUBLIC_SETUPS.get().unwrap(),
        )
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
        let _ = initialize_from_file_unchecked(
            &"../proof-of-sql/static-setups/public_parameters_nu_15"
                .parse()
                .unwrap(),
        );
        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let (_, insert_data) = sample_empty_and_populated_on_chain_table();

        let insert_data_bytes = OnChainTableBytes::try_from(insert_data.clone()).unwrap();

        let invalid_commitments = TableCommitmentBytesPerCommitmentSchemePassBy {
            data: TableCommitmentBytesPerCommitmentScheme {
                ipa: None,
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
        let _ = initialize_from_file_unchecked(
            &"../proof-of-sql/static-setups/public_parameters_nu_15"
                .parse()
                .unwrap(),
        );
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
