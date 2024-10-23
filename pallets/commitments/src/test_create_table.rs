use commitment_sql::{process_create_table, CreateTableAndCommitmentMetadata};
use on_chain_table::{OnChainColumn, OnChainTable};
use proof_of_sql::base::commitment::TableCommitment;
use proof_of_sql::proof_primitive::dory::{DoryScalar, DynamicDoryCommitment};
use proof_of_sql_commitment_map::{CommitmentScheme, CommitmentSchemeFlags, TableCommitmentBytes};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sxt_core::tables::TableIdentifier;

use crate::mock::*;
use crate::public_setups::PUBLIC_SETUPS;
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

        let empty_table = OnChainTable::try_from_iter([
            ("animal".parse().unwrap(), OnChainColumn::VarChar(vec![])),
            ("population".parse().unwrap(), OnChainColumn::BigInt(vec![])),
        ])
        .unwrap();

        let expected_commitment =
            TableCommitment::<DynamicDoryCommitment>::try_from_columns_with_offset(
                empty_table
                    .iter_committable::<DoryScalar>()
                    .map(Result::unwrap),
                0,
                &PUBLIC_SETUPS.dory,
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
            dory: true,
        };
        let (expected_create_table_and_commitment_metadata, _) =
            process_create_table(expected_create_table, *PUBLIC_SETUPS, &flags).unwrap();

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
            CommitmentsModule::table_commitment(&table_id, CommitmentScheme::Dory),
            Some(expected_commitment_bytes)
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
