use snafu::Snafu;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::Ident;

/// Prefix reserved for columns/tables that are for internal sxt-node usage.
pub const METADATA_PREFIX: &str = "META";

/// Metadata prefix is reserved for internal sxt-node usage.
#[derive(Debug, Snafu)]
#[snafu(display("{METADATA_PREFIX} prefix is reserved for internal sxt-node usage"))]
pub struct ReservedMetadataPrefix;

/// Returns `Ok(())` if none of the identifiers use the reserved metadata prefix.
fn validate_idents_avoid_prefix<'a>(
    columns: impl IntoIterator<Item = &'a Ident>,
) -> Result<(), ReservedMetadataPrefix> {
    columns
        .into_iter()
        .all(|ident| {
            !ident
                .value
                .to_ascii_uppercase()
                .starts_with(METADATA_PREFIX)
        })
        .then_some(())
        .ok_or(ReservedMetadataPrefix)
}

/// Returns `Ok(())` if neither the table nor column identifiers use the reserved metadata prefix.
pub fn validate_table_avoids_prefix(
    table: &CreateTableBuilder,
) -> Result<(), ReservedMetadataPrefix> {
    validate_idents_avoid_prefix(&table.name.0)
        .and_then(|_| validate_idents_avoid_prefix(table.columns.iter().map(|column| &column.name)))
}

#[cfg(test)]
mod tests {
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

    #[test]
    fn we_can_validate_tables_that_avoid_prefix() {
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

        assert!(validate_table_avoids_prefix(&create_table).is_ok());
    }

    #[test]
    fn we_cannot_validate_tables_that_use_prefix() {
        let create_table_with_reserved_column_prefix: CreateTableBuilder =
            Parser::new(&PostgreSqlDialect {})
                .try_with_sql(
                    "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            META_population BIGINT NOT NULL,
            PRIMARY KEY (animal))",
                )
                .unwrap()
                .parse_statement()
                .unwrap()
                .try_into()
                .unwrap();
        assert!(matches!(
            validate_table_avoids_prefix(&create_table_with_reserved_column_prefix),
            Err(ReservedMetadataPrefix)
        ));

        let create_table_with_reserved_table_name_prefix: CreateTableBuilder =
            Parser::new(&PostgreSqlDialect {})
                .try_with_sql(
                    "CREATE TABLE animal.meta_population (
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
            validate_table_avoids_prefix(&create_table_with_reserved_table_name_prefix),
            Err(ReservedMetadataPrefix)
        ));

        let create_table_with_reserved_namespace_prefix: CreateTableBuilder =
            Parser::new(&PostgreSqlDialect {})
                .try_with_sql(
                    "CREATE TABLE mEtanimal.population (
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
            validate_table_avoids_prefix(&create_table_with_reserved_namespace_prefix),
            Err(ReservedMetadataPrefix)
        ));
    }
}
