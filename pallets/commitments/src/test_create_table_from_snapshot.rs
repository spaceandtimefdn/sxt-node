use commitment_sql::{
    process_create_table,
    CreateTableAndCommitmentMetadata,
    OnChainTableToTableCommitmentFn,
};
use frame_support::assert_noop;
use on_chain_table::{OnChainColumn, OnChainTable};
use proof_of_sql_commitment_map::generic_over_commitment::{OptionType, TableCommitmentType};
use proof_of_sql_commitment_map::{
    CommitmentScheme,
    CommitmentSchemeFlags,
    PerCommitmentScheme,
    TableCommitmentBytesPerCommitmentScheme,
};
use proof_of_sql_static_setups::io::PUBLIC_SETUPS;
use sqlparser::ast::Ident;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sxt_core::tables::TableIdentifier;

use crate::mock::*;
use crate::test_create_table_generic::{self, CreateTableApiTestParams};
use crate::Error;

struct ProcessCreateTableFromSnapshotTestParams {
    sql_statement: String,
    snapshot_data: OnChainTable,
}

impl CreateTableApiTestParams for ProcessCreateTableFromSnapshotTestParams {
    fn new_valid() -> Self {
        let sql_statement = "CREATE TABLE animal.population (
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

        ProcessCreateTableFromSnapshotTestParams {
            sql_statement,
            snapshot_data,
        }
    }

    fn set_sql_statement(&mut self, sql_text: String) {
        self.sql_statement = sql_text;
    }

    fn execute(self) -> Result<CreateTableAndCommitmentMetadata, Error<Test>> {
        let commitments = PUBLIC_SETUPS
            .get()
            .unwrap()
            .into_iter()
            .map(|any| {
                any.map(OnChainTableToTableCommitmentFn::new(&self.snapshot_data, 0))
                    .transpose_result()
                    .unwrap()
            })
            .collect::<PerCommitmentScheme<OptionType<TableCommitmentType>>>();

        let per_commitment_scheme =
            TableCommitmentBytesPerCommitmentScheme::try_from(commitments).unwrap();

        let create_table = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&self.sql_statement)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        CommitmentsModule::process_create_table_from_snapshot_and_initiate_commitments(
            create_table,
            per_commitment_scheme,
        )
    }
}

#[test]
fn we_can_process_create_table_from_snapshot() {
    new_test_ext().execute_with(|| {
        let test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();

        let table_id = TableIdentifier {
            namespace: b"ANIMAL".to_vec().try_into().unwrap(),
            name: b"POPULATION".to_vec().try_into().unwrap(),
        };

        let expected_create_table = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&test_params.sql_statement)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let flags = CommitmentSchemeFlags {
            hyper_kzg: true,
            dynamic_dory: true,
        };
        let (expected_create_table_and_commitment_metadata, _) =
            process_create_table(expected_create_table, *PUBLIC_SETUPS.get().unwrap(), &flags)
                .unwrap();

        let expected_commitments = PUBLIC_SETUPS
            .get()
            .unwrap()
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

        let expected_commitments_bytes =
            TableCommitmentBytesPerCommitmentScheme::try_from(expected_commitments).unwrap();

        let create_table_and_commitment_metadata = test_params.execute().unwrap();

        assert_eq!(
            create_table_and_commitment_metadata,
            expected_create_table_and_commitment_metadata
        );
        assert_eq!(
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::DynamicDory),
            expected_commitments_bytes.dynamic_dory
        );
        assert_eq!(
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::HyperKzg),
            expected_commitments_bytes.hyper_kzg
        );
    });
}

#[test]
fn we_cannot_process_create_table_from_inappropriate_snapshot() {
    new_test_ext().execute_with(|| {
        // missing column
        let mut test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            PRIMARY KEY (animal))"
                .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::InappropriateSnapshotCommitments
        );

        // swapped columns
        let mut test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            population BIGINT NOT NULL,
            animal VARCHAR NOT NULL,
            PRIMARY KEY (animal))"
                .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::InappropriateSnapshotCommitments
        );

        // wrong type
        let mut test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population INT NOT NULL,
            PRIMARY KEY (animal))"
                .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::InappropriateSnapshotCommitments
        );

        // too many columns
        let mut test_params = ProcessCreateTableFromSnapshotTestParams::new_valid();
        test_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            class VARCHAR NOT NULL,
            PRIMARY KEY (animal))"
                .to_string(),
        );

        assert_noop!(
            test_params.execute(),
            Error::<Test>::InappropriateSnapshotCommitments
        );
    });
}

#[test]
fn we_cannot_process_invalid_create_table_from_snapshot() {
    test_create_table_generic::we_cannot_process_invalid_create_table::<
        ProcessCreateTableFromSnapshotTestParams,
    >()
}

#[test]
fn we_cannot_process_create_table_with_unsupported_column_from_snapshot() {
    test_create_table_generic::we_cannot_process_create_table_with_unsupported_column::<
        ProcessCreateTableFromSnapshotTestParams,
    >()
}

#[test]
fn we_cannot_process_create_table_from_snapshot_if_table_already_exists() {
    test_create_table_generic::we_cannot_process_create_table_if_table_already_exists::<
        ProcessCreateTableFromSnapshotTestParams,
    >()
}
