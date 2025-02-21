use on_chain_table::{OnChainColumn, OnChainTable};
use proof_of_sql::base::database::{ColumnType, TableRef};
use snafu::Snafu;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::{ColumnDef, ColumnOptionDef, Ident};

use crate::column_options::{validate_column_options, InvalidColumnOptions};
use crate::column_type_conversion::{
    sqlparser_data_type_to_proof_of_sql_column_type,
    UnsupportedColumnType,
};
use crate::map::IndexMap;
use crate::metadata_prefix::{validate_table_avoids_prefix, ReservedMetadataPrefix};

/// Error type for invalid table definitions.
#[derive(Debug, Snafu)]
pub enum InvalidCreateTable {
    /// Table must have at least one column.
    #[snafu(display("table must have at least one column"))]
    NoColumns,
    /// Table has unsupported column type.
    #[snafu(display("table has unsupported column type: {source}"), context(false))]
    UnsupportedColumnType {
        /// Source unsupported column type error.
        source: UnsupportedColumnType,
    },
    /// Table ref has unexpected number of idents.
    #[snafu(display("expected table ref with 2 identifiers, found {num_identifiers}"))]
    NumTableIdentifiers {
        /// The actual number of identifiers defined.
        num_identifiers: usize,
    },
    /// Table has duplicate identifiers.
    #[snafu(display("table has duplicate identifiers"))]
    DuplicateIdentifiers,
    /// Table uses reserved metadata prefix.
    #[snafu(transparent)]
    ReservedMetadataPrefix {
        /// Source reserved metadata prefix error.
        source: ReservedMetadataPrefix,
    },
    /// Table has invalid column options.
    #[snafu(display("table has invalid column options: {source}"), context(false))]
    ColumnOptions {
        /// Source invalid column options error.
        source: InvalidColumnOptions,
    },
}

/// Table definition validated for proof-of-sql usage.
///
/// Create statements can be invalid for a variety of reasons.
/// See [`InvalidCreateTable`] for an enumeration of these errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCreateTable<'a> {
    /// Reference to validated create table type.
    table: &'a CreateTableBuilder,
    /// Table identifier represented with proof-of-sql types
    ///
    /// Validation of the table identifier performs the conversion to this type.
    /// Storing it here is essentially caching - users can avoid performing that logic twice.
    proof_of_sql_table_identifier: TableRef,
    /// Schema represented with proof-of-sql types.
    ///
    /// Validation of the column types and identifiers performs the conversion to these types.
    /// Storing it here is essentially caching - users can avoid performing that logic twice.
    proof_of_sql_schema: IndexMap<&'a Ident, ColumnType>,
}

impl<'a> ValidatedCreateTable<'a> {
    /// Construct a [`ValidatedCreateTable`] by validating a table definition.
    pub fn validate(table: &'a CreateTableBuilder) -> Result<Self, InvalidCreateTable> {
        validate_table_avoids_prefix(table)?;

        table
            .columns
            .iter()
            .try_for_each(|ColumnDef { options, .. }| {
                validate_column_options(options.iter().map(|ColumnOptionDef { option, .. }| option))
            })?;

        let [schema_id, table_id] = table.name.0.as_slice() else {
            Err(InvalidCreateTable::NumTableIdentifiers {
                num_identifiers: table.name.0.len(),
            })?
        };

        let proof_of_sql_table_identifier =
            TableRef::from_idents(Some(schema_id.clone()), table_id.clone());

        let proof_of_sql_schema = table
            .columns
            .iter()
            .map(
                |ColumnDef {
                     name, data_type, ..
                 }| {
                    let column_type = sqlparser_data_type_to_proof_of_sql_column_type(data_type)?;
                    Ok((name, column_type))
                },
            )
            .collect::<Result<IndexMap<_, _>, InvalidCreateTable>>()?;

        if proof_of_sql_schema.is_empty() {
            return Err(InvalidCreateTable::NoColumns);
        }

        if proof_of_sql_schema.len() < table.columns.len() {
            return Err(InvalidCreateTable::DuplicateIdentifiers);
        }

        Ok(ValidatedCreateTable {
            table,
            proof_of_sql_table_identifier,
            proof_of_sql_schema,
        })
    }

    /// Immutable accessor to the validated table definition.
    pub fn table(&self) -> &CreateTableBuilder {
        self.table
    }

    /// Immutable accessor to the cached proof-of-sql table identifier.
    pub fn proof_of_sql_table_identifier(&self) -> &TableRef {
        &self.proof_of_sql_table_identifier
    }

    /// Immutable accessor to the cached proof-of-sql schema.
    pub fn proof_of_sql_schema(&self) -> &IndexMap<&Ident, ColumnType> {
        &self.proof_of_sql_schema
    }

    /// Consumes this table definition and produces an empty [`OnChainTable`] matching this schema.
    pub fn into_empty_table(self) -> OnChainTable {
        self.into()
    }
}

impl<'a> From<ValidatedCreateTable<'a>> for OnChainTable {
    fn from(value: ValidatedCreateTable<'a>) -> Self {
        OnChainTable::try_from_iter(value.proof_of_sql_schema.into_iter().map(
            |(identifier, column_type)| (identifier.clone(), OnChainColumn::empty_with_type(column_type)),
        )).expect("ValidatedCreateTable is guaranteed to have at least one column and that all columns have the same length")
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

    #[test]
    fn we_can_validate_table_definition() {
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

        let validated_create_table = ValidatedCreateTable::validate(&create_table).unwrap();

        assert_eq!(validated_create_table.table(), &create_table);

        assert_eq!(
            validated_create_table.proof_of_sql_table_identifier(),
            &"animal.population".parse().unwrap()
        );

        assert_eq!(
            validated_create_table.proof_of_sql_schema(),
            &IndexMap::from_iter([
                (&Ident::new("animal"), ColumnType::VarChar),
                (&Ident::new("population"), ColumnType::BigInt)
            ])
        );
    }

    #[test]
    fn we_cannot_validate_table_definition_with_no_columns() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql("CREATE TABLE animal.population ()")
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        assert!(matches!(
            ValidatedCreateTable::validate(&create_table),
            Err(InvalidCreateTable::NoColumns)
        ));
    }

    #[test]
    fn we_cannot_validate_table_definition_with_unsupported_column_type() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(
                "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population DOUBLE PRECISION NOT NULL,
            PRIMARY KEY (animal))",
            )
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        assert!(matches!(
            ValidatedCreateTable::validate(&create_table),
            Err(InvalidCreateTable::UnsupportedColumnType { .. })
        ));
    }

    #[test]
    fn we_cannot_validate_table_definition_with_invalid_identifier() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(
                "CREATE TABLE animal.population.last_year (
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
            ValidatedCreateTable::validate(&create_table),
            Err(InvalidCreateTable::NumTableIdentifiers { .. })
        ));
    }

    #[test]
    fn we_cannot_validate_table_definition_with_duplicate_identifiers() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(
                "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            animal BIGINT NOT NULL,
            PRIMARY KEY (animal))",
            )
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();
        assert!(matches!(
            ValidatedCreateTable::validate(&create_table),
            Err(InvalidCreateTable::DuplicateIdentifiers { .. })
        ));
    }

    #[test]
    fn we_cannot_validate_table_definition_with_reserved_metadata_prefix() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(
                "CREATE TABLE animal.population (
            metanimal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            PRIMARY KEY (animal))",
            )
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();
        assert!(matches!(
            ValidatedCreateTable::validate(&create_table),
            Err(InvalidCreateTable::ReservedMetadataPrefix { .. })
        ));
    }

    #[test]
    fn we_cannot_validate_table_definition_with_invalid_column_options() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(
                "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT,
            PRIMARY KEY (animal))",
            )
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();
        assert!(matches!(
            ValidatedCreateTable::validate(&create_table),
            Err(InvalidCreateTable::ColumnOptions { .. })
        ));
    }

    #[test]
    fn we_can_convert_validated_create_table_into_empty_table() {
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

        let empty_table = ValidatedCreateTable::validate(&create_table)
            .unwrap()
            .into_empty_table();

        let expected = OnChainTable::try_from_iter([
            (Ident::new("animal"), OnChainColumn::VarChar(vec![])),
            (Ident::new("population"), OnChainColumn::BigInt(vec![])),
        ])
        .unwrap();

        assert_eq!(empty_table, expected);
    }
}
