use commitment_sql::{process_create_table, CreateTableAndCommitmentMetadata};
use frame_support::assert_noop;
use on_chain_table::{OnChainColumn, OnChainTable};
use proof_of_sql::base::commitment::TableCommitment;
use proof_of_sql::proof_primitive::dory::{DoryScalar, DynamicDoryCommitment};
use proof_of_sql_commitment_map::{
    CommitmentScheme,
    CommitmentSchemeFlags,
    TableCommitmentBytes,
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
        let commitment = TableCommitment::<DynamicDoryCommitment>::try_from_columns_with_offset(
            self.snapshot_data
                .iter_committable::<DoryScalar>()
                .map(Result::unwrap),
            0,
            &PUBLIC_SETUPS.get().unwrap().dynamic_dory,
        )
        .unwrap();

        let commitment_bytes = TableCommitmentBytes::try_from(&commitment).unwrap();

        let per_commitment_scheme = TableCommitmentBytesPerCommitmentScheme {
            ipa: None,
            dynamic_dory: Some(commitment_bytes.clone()),
        };

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

        let expected_commitment =
            TableCommitment::<DynamicDoryCommitment>::try_from_columns_with_offset(
                test_params
                    .snapshot_data
                    .iter_committable::<DoryScalar>()
                    .map(Result::unwrap),
                0,
                &PUBLIC_SETUPS.get().unwrap().dynamic_dory,
            )
            .unwrap();

        let expected_commitment_bytes =
            TableCommitmentBytes::try_from(&expected_commitment).unwrap();

        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let expected_create_table = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&test_params.sql_statement)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let flags = CommitmentSchemeFlags {
            ipa: false,
            dynamic_dory: true,
        };
        let (expected_create_table_and_commitment_metadata, _) =
            process_create_table(expected_create_table, *PUBLIC_SETUPS.get().unwrap(), &flags)
                .unwrap();

        let create_table_and_commitment_metadata = test_params.execute().unwrap();

        assert_eq!(
            create_table_and_commitment_metadata,
            expected_create_table_and_commitment_metadata
        );
        assert_eq!(
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::Ipa),
            None
        );
        assert_eq!(
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::DynamicDory),
            Some(expected_commitment_bytes)
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
