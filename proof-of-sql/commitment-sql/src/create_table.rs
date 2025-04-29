use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

use on_chain_table::{OnChainTable, OutOfScalarBounds};
use proof_of_sql::base::commitment::{Commitment, TableCommitment};
use proof_of_sql_commitment_map::generic_over_commitment::{
    AssociatedPublicSetupType,
    GenericOverCommitment,
    OptionType,
    ResultOkType,
    TableCommitmentType,
};
use proof_of_sql_commitment_map::{
    CommitmentSchemeFlags,
    GenericOverCommitmentFn,
    PerCommitmentScheme,
};
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sxt_core::tables::TableIdentifier;

use crate::row_number_column::create_table_with_row_number_column;
use crate::validated_create_table::{InvalidCreateTable, ValidatedCreateTable};

/// Generically accepts a commitment setup and returns the table commitment to the captured
/// `OnChainTable` and offset.
pub struct OnChainTableToTableCommitmentFn<'a, 's>(&'a OnChainTable, usize, PhantomData<&'s ()>);

impl<'a> OnChainTableToTableCommitmentFn<'a, '_> {
    /// Construct a new [`OnChainTableToTableCommitmentFn`].
    pub fn new(table: &'a OnChainTable, offset: usize) -> Self {
        OnChainTableToTableCommitmentFn(table, offset, PhantomData)
    }
}

impl<'s> GenericOverCommitmentFn for OnChainTableToTableCommitmentFn<'_, 's> {
    type In = AssociatedPublicSetupType<'s>;
    type Out = ResultOkType<TableCommitmentType, OutOfScalarBounds>;

    fn call<C: Commitment>(
        &self,
        input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
    ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
        let committable_table = self
            .0
            .iter_committable::<C::Scalar>()
            .collect::<Result<Vec<_>, _>>()?;

        Ok(
            TableCommitment::try_from_columns_with_offset(committable_table, self.1, &input)
                .expect(
                    "OnChainTables cannot have columns of mixed length or duplicate identifiers",
                ),
        )
    }
}

/// Table definition transformed to support commitment metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTableAndCommitmentMetadata {
    /// The original table definition with additional meta columns.
    pub table_with_meta_columns: CreateTableBuilder,
    /// Definitions of metadata tables to be created alongside the original table.
    pub meta_tables: Vec<CreateTableBuilder>,
    /// Initial inserts to perform on metadata tables.
    pub meta_table_inserts: Vec<(TableIdentifier, OnChainTable)>,
}

/// Process table definition to support commitment metadata.
///
/// Returns..
/// - the processed table definition as [`CreateTableAndCommitmentMetadata`]
/// - the initial, empty commitments for the table
pub fn process_create_table(
    table: CreateTableBuilder,
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
    commitment_schemes: &CommitmentSchemeFlags,
) -> Result<
    (
        CreateTableAndCommitmentMetadata,
        PerCommitmentScheme<OptionType<TableCommitmentType>>,
    ),
    InvalidCreateTable,
> {
    let validated_create_table = ValidatedCreateTable::validate(&table)?;

    let empty_table = validated_create_table.into_empty_table();

    let empty_table_to_table_commitment = OnChainTableToTableCommitmentFn::new(&empty_table, 0);

    let empty_commitments = setups
        .select(commitment_schemes)
        .into_flat_iter()
        .map(|setup| {
            setup
                .map(&empty_table_to_table_commitment)
                .transpose_result()
                .expect("table is empty, therefore has no out-of-bounds values")
        })
        .collect();

    let table_with_meta_columns = create_table_with_row_number_column(table);

    Ok((
        CreateTableAndCommitmentMetadata {
            table_with_meta_columns,
            meta_tables: vec![],
            meta_table_inserts: vec![],
        },
        empty_commitments,
    ))
}

#[cfg(test)]
mod tests {
    use on_chain_table::OnChainColumn;
    use proof_of_sql::proof_primitive::dory::{DoryScalar, DynamicDoryCommitment};
    use proof_of_sql::proof_primitive::hyperkzg::{BNScalar, HyperKZGCommitment};
    use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
    use sqlparser::ast::Ident;
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

    #[test]
    fn we_can_process_create_table() {
        let setups = get_or_init_from_files_with_four_points_unchecked();

        // we currently cannot compute ipa commitments in no_std environments
        let flags = CommitmentSchemeFlags {
            dynamic_dory: true,
            hyper_kzg: true,
        };

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

        let expected_table_with_meta_columns: CreateTableBuilder =
            Parser::new(&PostgreSqlDialect {})
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

        let expected_dory_commitment =
            TableCommitment::<DynamicDoryCommitment>::try_from_columns_with_offset(
                OnChainTable::try_from_iter([
                    (Ident::new("animal"), OnChainColumn::VarChar(vec![])),
                    (Ident::new("population"), OnChainColumn::BigInt(vec![])),
                ])
                .unwrap()
                .iter_committable::<DoryScalar>()
                .map(|result| result.unwrap()),
                0,
                &setups.dynamic_dory,
            )
            .unwrap();

        let expected_hyper_kzg_commitment =
            TableCommitment::<HyperKZGCommitment>::try_from_columns_with_offset(
                OnChainTable::try_from_iter([
                    (Ident::new("animal"), OnChainColumn::VarChar(vec![])),
                    (Ident::new("population"), OnChainColumn::BigInt(vec![])),
                ])
                .unwrap()
                .iter_committable::<BNScalar>()
                .map(|result| result.unwrap()),
                0,
                &setups.hyper_kzg,
            )
            .unwrap();

        let expected_create_table_and_commitment_metadata = CreateTableAndCommitmentMetadata {
            table_with_meta_columns: expected_table_with_meta_columns,
            meta_tables: vec![],
            meta_table_inserts: vec![],
        };

        let expected_commitments = PerCommitmentScheme {
            hyper_kzg: Some(expected_hyper_kzg_commitment),
            dynamic_dory: Some(expected_dory_commitment),
        };

        assert_eq!(
            process_create_table(create_table, *setups, &flags).unwrap(),
            (
                expected_create_table_and_commitment_metadata,
                expected_commitments
            )
        );
    }

    #[test]
    fn we_cannot_process_invalid_create_table() {
        let setups = get_or_init_from_files_with_four_points_unchecked();

        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql("CREATE TABLE animal.population ()")
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let flags = CommitmentSchemeFlags::all();

        assert!(matches!(
            process_create_table(create_table, *setups, &flags),
            Err(InvalidCreateTable::NoColumns)
        ));
    }
}
