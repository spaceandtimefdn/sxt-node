use commitment_sql::{process_create_table, CreateTableAndCommitmentMetadata};
use proof_of_sql_commitment_map::{
    CommitmentScheme,
    CommitmentSchemeFlags,
    TableCommitmentBytesPerCommitmentScheme,
};
use proof_of_sql_static_setups::io::PUBLIC_SETUPS;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sxt_core::tables::TableIdentifier;

use crate::mock::*;
use crate::test_create_table_generic::{self, CreateTableApiTestParams};
use crate::Error;

/// Test parameters for process_create_table_and_initiate_commitments.
pub struct ProcessCreateTableTestParams {
    sql_statement: String,
}

impl CreateTableApiTestParams for ProcessCreateTableTestParams {
    fn new_valid() -> Self {
        let sql_statement = "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            PRIMARY KEY (animal))"
            .to_string();

        ProcessCreateTableTestParams { sql_statement }
    }

    fn set_sql_statement(&mut self, sql_text: String) {
        self.sql_statement = sql_text;
    }

    fn execute(self) -> Result<CreateTableAndCommitmentMetadata, Error<Test>> {
        let create_table = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&self.sql_statement)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        CommitmentsModule::process_create_table_and_initiate_commitments(create_table)
    }
}

#[test]
fn we_can_process_create_table() {
    new_test_ext().execute_with(|| {
        let test_params = ProcessCreateTableTestParams::new_valid();

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
        let (expected_create_table_and_commitment_metadata, expected_commitments) =
            process_create_table(expected_create_table, *PUBLIC_SETUPS.get().unwrap(), &flags)
                .unwrap();

        let expected_commitments_bytes =
            TableCommitmentBytesPerCommitmentScheme::try_from(expected_commitments).unwrap();

        let create_table_and_commitment_metadata = test_params.execute().unwrap();

        assert_eq!(
            create_table_and_commitment_metadata,
            expected_create_table_and_commitment_metadata
        );
        assert_eq!(
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::HyperKzg),
            expected_commitments_bytes.hyper_kzg
        );
        assert_eq!(
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::DynamicDory),
            expected_commitments_bytes.dynamic_dory
        );
    });
}

#[test]
fn we_cannot_process_invalid_create_table_from_snapshot() {
    test_create_table_generic::we_cannot_process_invalid_create_table::<ProcessCreateTableTestParams>(
    )
}

#[test]
fn we_cannot_process_create_table_with_unsupported_column_from_snapshot() {
    test_create_table_generic::we_cannot_process_create_table_with_unsupported_column::<
        ProcessCreateTableTestParams,
    >()
}

#[test]
fn we_cannot_process_create_table_if_table_already_exists() {
    test_create_table_generic::we_cannot_process_create_table_if_table_already_exists::<
        ProcessCreateTableTestParams,
    >()
}
