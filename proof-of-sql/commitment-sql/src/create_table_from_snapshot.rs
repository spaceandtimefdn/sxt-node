use proof_of_sql::base::commitment::TableCommitmentArithmeticError;
use proof_of_sql_commitment_map::generic_over_commitment::{
    AssociatedPublicSetupType,
    OptionType,
    PairType,
    ResultOkType,
    TableCommitmentType,
};
use proof_of_sql_commitment_map::{GenericOverCommitmentFn, PerCommitmentScheme};
use snafu::Snafu;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;

use crate::insert::OptionZipFn;
use crate::{process_create_table, CreateTableAndCommitmentMetadata, InvalidCreateTable};

/// Generically accepts a pair of `TableCommitment`s and tries to add them.
struct TryAddTableCommitmentsFn;

impl GenericOverCommitmentFn for TryAddTableCommitmentsFn {
    type In = PairType<TableCommitmentType, TableCommitmentType>;
    type Out = ResultOkType<TableCommitmentType, TableCommitmentArithmeticError>;

    fn call<C: proof_of_sql::base::commitment::Commitment>(
            &self,
            input: <Self::In as proof_of_sql_commitment_map::generic_over_commitment::GenericOverCommitment>::WithCommitment<C>,
    ) -> <Self::Out as proof_of_sql_commitment_map::generic_over_commitment::GenericOverCommitment>::WithCommitment<C>{
        input.0.try_add(input.1)
    }
}

/// Errors that can occur when processing a table creation from snapshot.
#[derive(Debug, Snafu)]
pub enum ProcessCreateTableFromSnapshotError {
    /// Invalid table definition.
    #[snafu(display("invalid table definition: {source}"), context(false))]
    InvalidCreateTable {
        /// Source invalid create table error.
        source: InvalidCreateTable,
    },
    /// Snapshot commitments don't match table definition.
    #[snafu(
        display("snapshot commitments don't match table definition: {source}"),
        context(false)
    )]
    InappropriateSnapshotCommitments {
        /// Source table commitment error.
        source: TableCommitmentArithmeticError,
    },
}

/// Process table definition to support commitment data with an initial snapshot commitment.
///
/// Returns..
/// - the processed table definition as [`CreateTableAndCommitmentMetadata`]
/// - the snapshot commitments (after validating they match the table definition)
pub fn process_create_table_from_snapshot(
    table: CreateTableBuilder,
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
    snapshot_commitments: PerCommitmentScheme<OptionType<TableCommitmentType>>,
) -> Result<
    (
        CreateTableAndCommitmentMetadata,
        PerCommitmentScheme<OptionType<TableCommitmentType>>,
    ),
    ProcessCreateTableFromSnapshotError,
> {
    let commitment_schemes = snapshot_commitments.to_flags();

    let (create_table_and_commitment_metadata, empty_commitments) =
        process_create_table(table, setups, &commitment_schemes)?;

    let validated_snapshot_commitments = empty_commitments
        .zip(snapshot_commitments)
        .map(OptionZipFn::new())
        .into_flat_iter()
        .map(|any| any.map(TryAddTableCommitmentsFn).transpose_result())
        .collect::<Result<_, TableCommitmentArithmeticError>>()?;

    Ok((
        create_table_and_commitment_metadata,
        validated_snapshot_commitments,
    ))
}

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};
    use alloc::vec;

    use on_chain_table::{OnChainColumn, OnChainTable};
    use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
    use sqlparser::ast::Ident;
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;
    use crate::create_table::OnChainTableToTableCommitmentFn;

    struct ProcessCreateTableFromSnapshotTestParams {
        sql_text: String,
        snapshot_data: OnChainTable,
        commitment_offset: usize,
    }

    impl ProcessCreateTableFromSnapshotTestParams {
        fn new_valid() -> Self {
            let sql_text = "CREATE TABLE animal.population (
                animal VARCHAR NOT NULL,
                population BIGINT NOT NULL,
                PRIMARY KEY (animal))"
                .to_string();

            let animals_col_id = Ident::new("animal");
            let animals_data = ["cow", "dog", "cat"].map(String::from);

            let population_col_id = Ident::new("population");
            let population_data = [100, 2, 7];

            let snapshot_data = OnChainTable::try_from_iter([
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

            let commitment_offset = 0;

            ProcessCreateTableFromSnapshotTestParams {
                sql_text,
                snapshot_data,
                commitment_offset,
            }
        }

        fn execute(
            self,
        ) -> Result<
            (
                CreateTableAndCommitmentMetadata,
                PerCommitmentScheme<OptionType<TableCommitmentType>>,
            ),
            ProcessCreateTableFromSnapshotError,
        > {
            let setups = get_or_init_from_files_with_four_points_unchecked();
            let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
                .try_with_sql(&self.sql_text)
                .unwrap()
                .parse_statement()
                .unwrap()
                .try_into()
                .unwrap();

            let snapshot_commitments = setups
                .into_iter()
                .map(|any| {
                    any.map(OnChainTableToTableCommitmentFn::new(
                        &self.snapshot_data,
                        self.commitment_offset,
                    ))
                    .transpose_result()
                    .unwrap()
                })
                .collect::<PerCommitmentScheme<OptionType<TableCommitmentType>>>();

            process_create_table_from_snapshot(create_table, *setups, snapshot_commitments)
        }
    }

    #[test]
    fn we_can_process_create_table_from_snapshot() {
        let setups = get_or_init_from_files_with_four_points_unchecked();

        let test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();

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

        let expected_create_table_and_commitment_metadata = CreateTableAndCommitmentMetadata {
            table_with_meta_columns: expected_table_with_meta_columns,
            meta_tables: vec![],
            meta_table_inserts: vec![],
        };

        let expected_snapshot_commitments = setups
            .into_iter()
            .map(|any| {
                any.map(OnChainTableToTableCommitmentFn::new(
                    &test_params.snapshot_data,
                    0,
                ))
                .transpose_result()
                .unwrap()
            })
            .collect::<PerCommitmentScheme<OptionType<TableCommitmentType>>>();

        assert_eq!(
            test_params.execute().unwrap(),
            (
                expected_create_table_and_commitment_metadata,
                expected_snapshot_commitments
            )
        );
    }

    #[test]
    fn we_cannot_process_invalid_create_table_from_snapshot() {
        let mut test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();

        test_params.sql_text = "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NULL,
            PRIMARY KEY (animal))"
            .to_string();

        assert!(matches!(
            test_params.execute(),
            Err(ProcessCreateTableFromSnapshotError::InvalidCreateTable { .. })
        ),);
    }

    #[test]
    fn we_cannot_process_create_table_with_noncontiguous_snapshot() {
        let mut test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();

        test_params.commitment_offset = 1;

        assert!(matches!(
            test_params.execute(),
            Err(ProcessCreateTableFromSnapshotError::InappropriateSnapshotCommitments { .. })
        ),);
    }

    #[test]
    fn we_cannot_process_create_table_with_mismatched_snapshot() {
        let mut test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();

        let animals_col_id = Ident::new("animal");
        let animals_data = ["cow", "dog", "cat"].map(String::from);

        let population_col_id = Ident::new("population");
        let population_data = [100, 2, 7];

        test_params.snapshot_data = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::VarChar(animals_data.to_vec()),
            ),
            (
                population_col_id,
                OnChainColumn::Int(population_data.to_vec()),
            ),
        ])
        .unwrap();

        assert!(matches!(
            test_params.execute(),
            Err(ProcessCreateTableFromSnapshotError::InappropriateSnapshotCommitments { .. })
        ),);
    }
}
