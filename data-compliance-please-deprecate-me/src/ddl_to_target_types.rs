use indexmap::IndexMap;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::{DataType, ObjectName};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

/// For a single table definition, get a mapping of column names to types.
pub fn target_types_for_table(create_table: CreateTableBuilder) -> IndexMap<String, DataType> {
    create_table
        .columns
        .into_iter()
        .map(|column_def| (column_def.name.value, column_def.data_type))
        .collect()
}

/// For the contents of a sql file, get a mapping of table identifiers, to column names to types.
pub fn all_target_types_for_sql(sql: &str) -> IndexMap<ObjectName, IndexMap<String, DataType>> {
    let dialect = PostgreSqlDialect {};
    let ast = Parser::parse_sql(&dialect, sql).expect("Failed to parse SQL");

    ast.into_iter()
        .filter_map(|statement| {
            CreateTableBuilder::try_from(statement)
                .ok()
                .map(|builder| (builder.name.clone(), target_types_for_table(builder)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::hash::RandomState;

    use sqlparser::ast::{Ident, TimezoneInfo};

    use super::*;

    #[test]
    fn test_find_bigdecimals() {
        let sql = "
        CREATE SCHEMA IF NOT EXISTS ETHEREUM;

        CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS(
            BLOCK_NUMBER BIGINT NOT NULL,
            REWARD DECIMAL(78, 0),
          );
          
        CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCK_DETAILS(
            TIME_STAMP TIMESTAMP,
            SHA3_UNCLES VARCHAR,
        );";

        let expected = IndexMap::<_, _, RandomState>::from_iter([
            (
                ObjectName(vec![Ident::new("ETHEREUM"), Ident::new("BLOCKS")]),
                IndexMap::<_, _, RandomState>::from_iter([
                    ("BLOCK_NUMBER".to_string(), DataType::BigInt(None)),
                    (
                        "REWARD".to_string(),
                        DataType::Decimal(sqlparser::ast::ExactNumberInfo::PrecisionAndScale(
                            78, 0,
                        )),
                    ),
                ]),
            ),
            (
                ObjectName(vec![Ident::new("ETHEREUM"), Ident::new("BLOCK_DETAILS")]),
                IndexMap::<_, _, RandomState>::from_iter([
                    (
                        "TIME_STAMP".to_string(),
                        DataType::Timestamp(None, TimezoneInfo::None),
                    ),
                    ("SHA3_UNCLES".to_string(), DataType::Varchar(None)),
                ]),
            ),
        ]);

        assert_eq!(all_target_types_for_sql(sql), expected);
    }
}
