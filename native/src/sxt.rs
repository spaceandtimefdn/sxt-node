//! The native code implementation
#[cfg(feature = "std")]
use arrow::ipc::reader::StreamReader;
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
use postcard::to_allocvec;
use sp_runtime::BoundedVec;
use sp_runtime_interface::runtime_interface;
use sxt_core::native::{CreateStatementPassBy, NativeError, OnChainTableBytes, RowData};
use sxt_core::tables::create_statement_to_sqlparser;

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

        let table_bytes =
            to_allocvec(&on_chain_table).map_err(|_| NativeError::SerializationError)?;

        let table_bytes: BoundedVec<u8, _> =
            BoundedVec::try_from(table_bytes).map_err(|_| NativeError::BoundedVecError)?;

        Ok(OnChainTableBytes { data: table_bytes })
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

        let table_bytes =
            to_allocvec(&on_chain_table).map_err(|_| NativeError::SerializationError)?;

        let table_bytes: BoundedVec<u8, _> =
            BoundedVec::try_from(table_bytes).map_err(|_| NativeError::BoundedVecError)?;

        Ok(OnChainTableBytes { data: table_bytes })
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;
    use std::io::Cursor;
    use std::sync::Arc;

    use arrow::array::{ArrayRef, Int32Array, RecordBatch, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::ipc::writer::StreamWriter;
    use on_chain_table::{OnChainColumn, OnChainTable};
    use proof_of_sql::base::math::decimal::Precision;
    use sp_core::U256;
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
            RecordBatch::try_from_iter([("id", id_column), ("price", price_column)]).unwrap();

        let mut buffer: Vec<u8> = Vec::new();
        let mut writer = StreamWriter::try_new(&mut buffer, &illegal_batch.schema()).unwrap();
        writer.write(&illegal_batch).unwrap();
        writer.finish().unwrap();

        let illegal_batch_bytes = RowData {
            row_data: BoundedVec::try_from(buffer).unwrap(),
        };

        let create_statement = CreateStatementPassBy {
            create_statement: create_statement(
                "CREATE TABLE test.table (id VARCHAR NULL, price DECIMAL(78, 0) NULL)",
            ),
        };

        let expected_on_chain_table = OnChainTable::try_from_iter([
            (
                "id".parse().unwrap(),
                OnChainColumn::VarChar(
                    ["lorem", "ipsum", "", "dolor"]
                        .map(ToString::to_string)
                        .to_vec(),
                ),
            ),
            (
                "price".parse().unwrap(),
                OnChainColumn::Decimal75(Precision::new(75).unwrap(), 0, vec![U256::zero(), U256::zero(), U256::MAX - U256::from(999), U256::from_str_radix("896044618658097711785492504343953926634992332820282019728792003956564819967", 10).unwrap()])
            ),
        ]).unwrap();

        let result: OnChainTable = postcard::from_bytes(
            &interface::record_batch_to_onchain(illegal_batch_bytes, create_statement)
                .unwrap()
                .data,
        )
        .unwrap();

        assert_eq!(result, expected_on_chain_table);
    }
}
