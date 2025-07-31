use indexmap::IndexMap;
use proof_of_sql::base::database::{ColumnType, SchemaAccessor, TableRef};
use snafu::Snafu;
use sqlparser::ast::{ColumnDef, Ident};
use sqlparser::parser::ParserError;
use sxt_core::tables::TableSchema;

use super::column_type_conversion::{
    sqlparser_data_type_to_proof_of_sql_column_type,
    UnsupportedColumnType,
};

#[derive(Debug, Snafu)]
pub enum TableToProofOfSqlSchemaError {
    #[snafu(display("unable to parse data type: {source}"), context(false))]
    Parser { source: ParserError },
    #[snafu(display("column type is not supported: {source}"), context(false))]
    Unsupported { source: UnsupportedColumnType },
}

fn table_schema_to_proof_of_sql_schema(
    table_schema: TableSchema,
) -> Result<IndexMap<Ident, ColumnType>, TableToProofOfSqlSchemaError> {
    table_schema
        .into_iter()
        .map(|column_schema| {
            let ColumnDef {
                name, data_type, ..
            } = column_schema.try_into()?;

            let column_type = sqlparser_data_type_to_proof_of_sql_column_type(&data_type)?;

            Ok((name, column_type))
        })
        .collect()
}

/// A simple proof-of-sql `SchemaAccessor` that can be built from on-chain `TableSchema`s.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct QuerySchema {
    schemas: IndexMap<TableRef, IndexMap<Ident, ColumnType>>,
}

impl QuerySchema {
    /// Consruct a new `QuerySchema` from `TableSchema`s.
    pub fn try_from_table_schemas(
        table_schemas: impl IntoIterator<Item = (TableRef, TableSchema)>,
    ) -> Result<QuerySchema, TableToProofOfSqlSchemaError> {
        table_schemas
            .into_iter()
            .map(|(table_ref, table_schema)| {
                table_schema_to_proof_of_sql_schema(table_schema).map(|schema| (table_ref, schema))
            })
            .collect::<Result<IndexMap<_, _>, _>>()
            .map(|schemas| QuerySchema { schemas })
    }
}

impl SchemaAccessor for QuerySchema {
    fn lookup_column(&self, table_ref: &TableRef, column_id: &Ident) -> Option<ColumnType> {
        self.schemas.get(table_ref)?.get(column_id).copied()
    }

    fn lookup_schema(&self, table_ref: &TableRef) -> Vec<(Ident, ColumnType)> {
        self.schemas
            .get(table_ref)
            .unwrap_or(&IndexMap::default())
            .iter()
            .map(|(id, col)| (id.clone(), *col))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use proof_of_sql::base::math::decimal::Precision;
    use proof_of_sql::base::posql_time::{PoSQLTimeUnit, PoSQLTimeZone};
    use sqlparser::ast::{DataType, ExactNumberInfo, TimezoneInfo};
    use sxt_core::tables::ScaleColumnSchema;

    use super::*;

    #[test]
    fn we_can_convert_table_schema_to_proof_of_sql_schema() {
        let table_schema = [
            (Ident::new("VARCHAR_COL"), DataType::Varchar(None)),
            (Ident::new("TINYINT_COL"), DataType::TinyInt(None)),
            (Ident::new("SMALLINT_COL"), DataType::SmallInt(None)),
            (Ident::new("INT_COL"), DataType::Int(None)),
            (Ident::new("BIGINT_COL"), DataType::BigInt(None)),
            (
                Ident::new("DECIMAL_COL"),
                DataType::Decimal(ExactNumberInfo::PrecisionAndScale(10, 2)),
            ),
            (Ident::new("BOOLEAN_COL"), DataType::Boolean),
            (
                Ident::new("TIMESTAMP_COL"),
                DataType::Timestamp(None, TimezoneInfo::None),
            ),
            (Ident::new("BINARY_COL"), DataType::Binary(None)),
        ]
        .map(|(name, data_type)| ColumnDef {
            name,
            data_type,
            options: Vec::new(),
            collation: None,
        })
        .map(ScaleColumnSchema::try_from)
        .map(Result::unwrap)
        .to_vec();

        let query_schema =
            QuerySchema::try_from_table_schemas([("TEST.TABLE1".parse().unwrap(), table_schema)])
                .unwrap();

        let expected = QuerySchema {
            schemas: IndexMap::from_iter([(
                "TEST.TABLE1".parse().unwrap(),
                IndexMap::from_iter([
                    (Ident::new("VARCHAR_COL"), ColumnType::VarChar),
                    (Ident::new("TINYINT_COL"), ColumnType::TinyInt),
                    (Ident::new("SMALLINT_COL"), ColumnType::SmallInt),
                    (Ident::new("INT_COL"), ColumnType::Int),
                    (Ident::new("BIGINT_COL"), ColumnType::BigInt),
                    (
                        Ident::new("DECIMAL_COL"),
                        ColumnType::Decimal75(Precision::new(10).unwrap(), 2),
                    ),
                    (Ident::new("BOOLEAN_COL"), ColumnType::Boolean),
                    (
                        Ident::new("TIMESTAMP_COL"),
                        ColumnType::TimestampTZ(PoSQLTimeUnit::Millisecond, PoSQLTimeZone::utc()),
                    ),
                    (Ident::new("BINARY_COL"), ColumnType::VarBinary),
                ]),
            )]),
        };

        assert_eq!(query_schema, expected);
    }

    #[test]
    fn we_cannot_convert_table_schema_with_unsupported_type_to_proof_of_sql_schema() {
        let table_schema = [
            (Ident::new("VARCHAR_COL"), DataType::Varchar(None)),
            (Ident::new("FLOAT_COL"), DataType::Float(None)),
        ]
        .map(|(name, data_type)| ColumnDef {
            name,
            data_type,
            options: Vec::new(),
            collation: None,
        })
        .map(ScaleColumnSchema::try_from)
        .map(Result::unwrap)
        .to_vec();

        let result =
            QuerySchema::try_from_table_schemas([("TEST.TABLE1".parse().unwrap(), table_schema)]);

        assert!(matches!(
            result,
            Err(TableToProofOfSqlSchemaError::Unsupported { .. })
        ));
    }

    #[test]
    fn we_can_use_query_schema_as_schema_accessor() {
        let table_1_ref: TableRef = "TEST.TABLE1".parse().unwrap();
        let table_2_ref: TableRef = "TEST.TABLE2".parse().unwrap();

        let column_defs = [
            (Ident::new("VARCHAR_COL"), DataType::Varchar(None)),
            (Ident::new("INT_COL"), DataType::Int(None)),
            (Ident::new("BIGINT_COL"), DataType::BigInt(None)),
            (Ident::new("BINARY_COL"), DataType::Binary(None)),
        ]
        .map(|(name, data_type)| ColumnDef {
            name,
            data_type,
            options: Vec::new(),
            collation: None,
        });

        let columns = column_defs
            .clone()
            .map(ScaleColumnSchema::try_from)
            .map(Result::unwrap);

        let table_1_schema = columns[0..2].to_vec();
        let table_2_schema = columns[2..4].to_vec();

        let query_schema = QuerySchema::try_from_table_schemas([
            (table_1_ref.clone(), table_1_schema.clone()),
            (table_2_ref.clone(), table_2_schema.clone()),
        ])
        .unwrap();

        assert_eq!(
            query_schema.lookup_schema(&table_1_ref),
            vec![
                (column_defs[0].name.clone(), ColumnType::VarChar),
                (column_defs[1].name.clone(), ColumnType::Int)
            ]
        );
        assert_eq!(
            query_schema.lookup_column(&table_1_ref, &column_defs[0].name),
            Some(ColumnType::VarChar)
        );
        assert_eq!(
            query_schema.lookup_column(&table_1_ref, &column_defs[1].name),
            Some(ColumnType::Int)
        );
        assert_eq!(
            query_schema.lookup_column(&table_1_ref, &column_defs[2].name),
            None
        );

        assert_eq!(
            query_schema.lookup_schema(&table_2_ref),
            vec![
                (column_defs[2].name.clone(), ColumnType::BigInt),
                (column_defs[3].name.clone(), ColumnType::VarBinary)
            ]
        );
        assert_eq!(
            query_schema.lookup_column(&table_2_ref, &column_defs[2].name),
            Some(ColumnType::BigInt)
        );
        assert_eq!(
            query_schema.lookup_column(&table_2_ref, &column_defs[3].name),
            Some(ColumnType::VarBinary)
        );
        assert_eq!(
            query_schema.lookup_column(&table_2_ref, &column_defs[0].name),
            None
        );
    }
}
