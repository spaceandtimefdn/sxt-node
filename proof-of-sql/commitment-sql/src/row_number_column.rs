use alloc::vec;
use alloc::vec::Vec;

use const_format::formatcp;
use on_chain_table::{OnChainColumn, OnChainTable};
use snafu::Snafu;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::{ColumnDef, ColumnOption, ColumnOptionDef, DataType, Ident, TableConstraint};

/// Row number column name.
const ROW_NUMBER_COLUMN_NAME: &str = "META_ROW_NUMBER";

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

/// Adds a entry for the META_ROW_NUMBER column to the list of primary keys for the provided table
pub fn add_row_number_primary_key(mut table: CreateTableBuilder) -> CreateTableBuilder {
    if table
        .constraints
        .iter()
        .any(|c| matches!(c, sqlparser::ast::TableConstraint::PrimaryKey { .. }))
    {
        // Add the meta row number to primary key as well
        table.constraints = table
            .constraints
            .into_iter()
            .map(|c: TableConstraint| match c {
                sqlparser::ast::TableConstraint::PrimaryKey {
                    name,
                    index_name,
                    index_type,
                    mut columns,
                    index_options,
                    characteristics,
                } => {
                    columns.push(Ident::new(ROW_NUMBER_COLUMN_NAME));
                    sqlparser::ast::TableConstraint::PrimaryKey {
                        name,
                        index_name,
                        index_type,
                        columns,
                        index_options,
                        characteristics,
                    }
                }
                c => c,
            })
            .collect();
    } else {
        table
            .constraints
            .push(sqlparser::ast::TableConstraint::PrimaryKey {
                name: None,
                index_name: None,
                index_type: None,
                columns: vec![Ident::new(ROW_NUMBER_COLUMN_NAME)],
                index_options: Vec::new(),
                characteristics: None,
            });
    }

    table
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

/// Metadata prefix is reserved for internal sxt-node usage.
#[derive(Debug, Snafu)]
#[snafu(display("{ROW_NUMBER_COLUMN_NAME} prefix is reserved for internal sxt-node usage"))]
pub struct ReservedMetaRowNumberColumnName;

/// Returns `Ok(())` if none of the identifiers use the reserved metadata prefix.
fn validate_idents_avoid_row_number_column_name<'a>(
    columns: impl IntoIterator<Item = &'a Ident>,
) -> Result<(), ReservedMetaRowNumberColumnName> {
    columns
        .into_iter()
        .all(|ident| ident.value.to_ascii_uppercase() != ROW_NUMBER_COLUMN_NAME)
        .then_some(())
        .ok_or(ReservedMetaRowNumberColumnName)
}

/// Returns `Ok(())` if neither the table nor column identifiers use the reserved metadata prefix.
pub fn validate_table_avoids_row_number_column_name(
    table: &CreateTableBuilder,
) -> Result<(), ReservedMetaRowNumberColumnName> {
    validate_idents_avoid_row_number_column_name(&table.name.0).and_then(|_| {
        validate_idents_avoid_row_number_column_name(
            table.columns.iter().map(|column| &column.name),
        )
    })
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

    #[test]
    fn we_can_inject_meta_row_as_primary_key() {
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
            PRIMARY KEY (animal, META_ROW_NUMBER))",
            )
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        assert_eq!(add_row_number_primary_key(create_table), expected);
    }

    #[test]
    fn we_can_inject_meta_row_as_primary_key_to_table_with_no_pk() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(
                "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL)",
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
            PRIMARY KEY (META_ROW_NUMBER))",
            )
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        assert_eq!(add_row_number_primary_key(create_table), expected);
    }

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

    #[test]
    fn we_can_validate_tables_that_avoid_row_number_column_name() {
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

        assert!(validate_table_avoids_row_number_column_name(&create_table).is_ok());
    }

    #[test]
    fn we_cannot_validate_tables_that_use_row_number_column_name() {
        let create_table_with_reserved_column_name: CreateTableBuilder =
            Parser::new(&PostgreSqlDialect {})
                .try_with_sql(
                    "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            meta_row_number BIGINT NOT NULL,
            PRIMARY KEY (animal))",
                )
                .unwrap()
                .parse_statement()
                .unwrap()
                .try_into()
                .unwrap();
        assert!(matches!(
            validate_table_avoids_row_number_column_name(&create_table_with_reserved_column_name),
            Err(ReservedMetaRowNumberColumnName)
        ));

        let create_table_with_reserved_table_name: CreateTableBuilder =
            Parser::new(&PostgreSqlDialect {})
                .try_with_sql(
                    "CREATE TABLE animal.meta_row_number (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            PRIMARY KEY (animal))",
                )
                .unwrap()
                .parse_statement()
                .unwrap()
                .try_into()
                .unwrap();
        assert!(matches!(
            validate_table_avoids_row_number_column_name(&create_table_with_reserved_table_name),
            Err(ReservedMetaRowNumberColumnName)
        ));

        let create_table_with_reserved_namespace: CreateTableBuilder =
            Parser::new(&PostgreSqlDialect {})
                .try_with_sql(
                    "CREATE TABLE mEta_row_number.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            PRIMARY KEY (animal))",
                )
                .unwrap()
                .parse_statement()
                .unwrap()
                .try_into()
                .unwrap();
        assert!(matches!(
            validate_table_avoids_row_number_column_name(&create_table_with_reserved_namespace),
            Err(ReservedMetaRowNumberColumnName)
        ));
    }
}
