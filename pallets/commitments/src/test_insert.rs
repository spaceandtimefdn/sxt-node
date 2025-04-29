use commitment_sql::{
    process_insert,
    InsertAndCommitmentMetadata,
    OnChainTableToTableCommitmentFn,
};
use frame_support::assert_noop;
use native_api::Api;
use on_chain_table::{OnChainColumn, OnChainTable};
use proof_of_sql::base::math::decimal::Precision;
use proof_of_sql_commitment_map::generic_over_commitment::{OptionType, TableCommitmentType};
use proof_of_sql_commitment_map::{
    CommitmentScheme,
    PerCommitmentScheme,
    TableCommitmentBytesPerCommitmentScheme,
};
use proof_of_sql_static_setups::io::PUBLIC_SETUPS;
use sp_core::U256;
use sqlparser::ast::Ident;
use sxt_core::tables::TableIdentifier;

use crate::mock::{new_test_ext, CommitmentsModule, Test};
use crate::test_create_table::ProcessCreateTableTestParams;
use crate::test_create_table_generic::CreateTableApiTestParams;
use crate::Error;

struct ProcessInsertTestParams {
    table_id: TableIdentifier,
    insert_data: OnChainTable,
}

impl ProcessInsertTestParams {
    fn new_valid() -> Self {
        let table_id = TableIdentifier {
            namespace: b"ANIMAL".to_vec().try_into().unwrap(),
            name: b"POPULATION".to_vec().try_into().unwrap(),
        };

        let animals_col_id = Ident::new("animal");
        let animals_data = ["cow", "dog", "cat"].map(String::from);

        let population_col_id = Ident::new("population");
        let population_data = [100, 2, 7];

        let insert_data = OnChainTable::try_from_iter([
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

        ProcessInsertTestParams {
            table_id,
            insert_data,
        }
    }

    fn execute(self) -> Result<InsertAndCommitmentMetadata, Error<Test>> {
        CommitmentsModule::process_insert_and_update_commitments::<Api>(
            self.table_id,
            self.insert_data,
        )
    }
}

#[test]
fn we_can_process_insert() {
    new_test_ext().execute_with(|| {
        ProcessCreateTableTestParams::new_valid().execute().unwrap();

        let empty_table = OnChainTable::try_from_iter([
            (Ident::new("animal"), OnChainColumn::VarChar(vec![])),
            (Ident::new("population"), OnChainColumn::BigInt(vec![])),
        ])
        .unwrap();

        let empty_commitments = PUBLIC_SETUPS
            .get()
            .unwrap()
            .into_iter()
            .map(|any| {
                any.map(OnChainTableToTableCommitmentFn::new(&empty_table, 0))
                    .transpose_result()
                    .unwrap()
            })
            .collect::<PerCommitmentScheme<OptionType<TableCommitmentType>>>();

        let test_params = ProcessInsertTestParams::new_valid();

        let (expected_insert_and_commitment_metadata, expected_commitments) = process_insert(
            &test_params.table_id,
            test_params.insert_data.clone(),
            empty_commitments,
            *PUBLIC_SETUPS.get().unwrap(),
        )
        .unwrap();
        let expected_commitments_bytes =
            TableCommitmentBytesPerCommitmentScheme::try_from(expected_commitments).unwrap();

        let table_id = test_params.table_id.clone();
        let insert_and_commitment_metadata = test_params.execute().unwrap();

        assert_eq!(
            insert_and_commitment_metadata,
            expected_insert_and_commitment_metadata
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
fn we_cannot_process_inserts_for_nonexistent_table() {
    new_test_ext().execute_with(|| {
        // table needs to be created first
        assert_noop!(
            ProcessInsertTestParams::new_valid().execute(),
            Error::<Test>::NoExistingCommitments
        );
    });
}

#[test]
fn we_cannot_process_inserts_for_table_with_out_of_bounds_values() {
    new_test_ext().execute_with(|| {
        let mut create_table_params = ProcessCreateTableTestParams::new_valid();

        create_table_params.set_sql_statement(
            "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population DECIMAL(75, 0) NOT NULL,
            PRIMARY KEY (animal))"
                .to_string(),
        );
        create_table_params.execute().unwrap();

        let mut insert_params = ProcessInsertTestParams::new_valid();
        insert_params.insert_data = OnChainTable::try_from_iter([
            (
                Ident::new("animal"),
                OnChainColumn::VarChar(vec!["Water bear".to_string()]),
            ),
            (
                Ident::new("population"),
                OnChainColumn::Decimal75(Precision::new(75).unwrap(), 0, vec![U256::MAX / 2]),
            ),
        ])
        .unwrap();

        assert_noop!(
            insert_params.execute(),
            Error::<Test>::InsertDataOutOfBounds
        );
    });
}

#[test]
fn we_cannot_process_inserts_that_dont_match_table() {
    new_test_ext().execute_with(|| {
        ProcessCreateTableTestParams::new_valid().execute().unwrap();

        // missing column
        let mut insert_params = ProcessInsertTestParams::new_valid();
        insert_params.insert_data = OnChainTable::try_from_iter([(
            Ident::new("animal"),
            OnChainColumn::VarChar(vec!["cow".to_string()]),
        )])
        .unwrap();

        assert_noop!(
            insert_params.execute(),
            Error::<Test>::InsertDataDoesntMatchExistingCommitments
        );

        // too many columns
        let mut insert_params = ProcessInsertTestParams::new_valid();
        insert_params.insert_data = OnChainTable::try_from_iter([
            (
                Ident::new("animal"),
                OnChainColumn::VarChar(vec!["cow".to_string()]),
            ),
            (Ident::new("population"), OnChainColumn::BigInt(vec![100])),
            (
                Ident::new("class"),
                OnChainColumn::VarChar(vec!["mammalia".to_string()]),
            ),
        ])
        .unwrap();

        assert_noop!(
            insert_params.execute(),
            Error::<Test>::InsertDataDoesntMatchExistingCommitments
        );

        // column of incorrect type
        let mut insert_params = ProcessInsertTestParams::new_valid();
        insert_params.insert_data = OnChainTable::try_from_iter([
            (
                Ident::new("animal"),
                OnChainColumn::VarChar(vec!["cow".to_string()]),
            ),
            (Ident::new("population"), OnChainColumn::Int(vec![100])),
        ])
        .unwrap();

        assert_noop!(
            insert_params.execute(),
            Error::<Test>::InsertDataDoesntMatchExistingCommitments
        );
    });
}
