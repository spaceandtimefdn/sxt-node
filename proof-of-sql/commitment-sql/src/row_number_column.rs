use alloc::vec;
use alloc::vec::Vec;

use const_format::formatcp;
use on_chain_table::{OnChainColumn, OnChainTable};
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::{ColumnDef, ColumnOption, ColumnOptionDef, DataType, Ident};

use crate::metadata_prefix::METADATA_PREFIX;

/// Suffix used for the row number column name.
const ROW_NUMBER_COLUMN_NAME_SUFFIX: &str = "ROW_NUMBER";

/// Row number column name.
const ROW_NUMBER_COLUMN_NAME: &str = formatcp!("{METADATA_PREFIX}_{ROW_NUMBER_COLUMN_NAME_SUFFIX}");

/// Returns a sqlparser `ColumnDef` for the row number column.
pub fn row_number_column_def() -> ColumnDef {
    ColumnDef {
        name: Ident::new(ROW_NUMBER_COLUMN_NAME),
        data_type: DataType::BigInt(None),
        collation: None,
        options: vec![ColumnOptionDef {
            name: None,
            option: ColumnOption::NotNull,
        }],
    }
}

/// Pushes a bigint row number metadata column onto the table definition.
pub fn create_table_with_row_number_column(mut table: CreateTableBuilder) -> CreateTableBuilder {
    table.columns.push(row_number_column_def());

    table
}

/// Pushes a bigint row number metadata column onto the `OnChainTable`.
///
/// The values for this column increment the rows, starting with `row_number_offset`.
pub fn on_chain_table_with_row_number_column(
    table: OnChainTable,
    row_number_offset: usize,
) -> OnChainTable {
    let row_number_column = OnChainColumn::BigInt(Vec::from_iter(
        row_number_offset as i64..row_number_offset as i64 + table.num_rows() as i64,
    ));

    OnChainTable::try_from_iter(table.into_iter().chain(core::iter::once((
        Ident::new(ROW_NUMBER_COLUMN_NAME),
        row_number_column,
    ))))
    .expect(
        "OnChainTable type and row_number_column construction guarantee matching column lengths",
    )
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

    #[test]
    fn we_can_transform_create_table_with_row_number_column() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(
                "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            PRIMARY KEY (animal))",
            )
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let expected: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(
                "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            META_ROW_NUMBER BIGINT NOT NULL,
            PRIMARY KEY (animal))",
            )
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        assert_eq!(create_table_with_row_number_column(create_table), expected);
    }

    #[test]
    fn we_can_transform_on_chain_table_with_row_number_column() {
        let data = [
            (
                Ident::new("animal"),
                OnChainColumn::VarChar(["cow", "dog", "cat"].map(String::from).to_vec()),
            ),
            (
                Ident::new("population"),
                OnChainColumn::BigInt(vec![100, 2, 7]),
            ),
        ];

        let on_chain_table = OnChainTable::try_from_iter(data.clone()).unwrap();
        let expected_from_0 =
            OnChainTable::try_from_iter(data.clone().into_iter().chain(core::iter::once((
                Ident::new("META_ROW_NUMBER"),
                OnChainColumn::BigInt(vec![0, 1, 2]),
            ))))
            .unwrap();
        assert_eq!(
            on_chain_table_with_row_number_column(on_chain_table.clone(), 0),
            expected_from_0
        );

        let expected_from_3 =
            OnChainTable::try_from_iter(data.into_iter().chain(core::iter::once((
                Ident::new("META_ROW_NUMBER"),
                OnChainColumn::BigInt(vec![3, 4, 5]),
            ))))
            .unwrap();
        assert_eq!(
            on_chain_table_with_row_number_column(on_chain_table, 3),
            expected_from_3
        );
    }

    #[test]
    fn we_can_transform_empty_on_chain_table_with_row_number_column() {
        let data = [
            (Ident::new("animal"), OnChainColumn::VarChar(vec![])),
            (Ident::new("population"), OnChainColumn::BigInt(vec![])),
        ];

        let on_chain_table = OnChainTable::try_from_iter(data.clone()).unwrap();
        let expected = OnChainTable::try_from_iter(data.into_iter().chain(core::iter::once((
            Ident::new("META_ROW_NUMBER"),
            OnChainColumn::BigInt(vec![]),
        ))))
        .unwrap();
        assert_eq!(
            on_chain_table_with_row_number_column(on_chain_table, 0),
            expected
        );
    }
}
