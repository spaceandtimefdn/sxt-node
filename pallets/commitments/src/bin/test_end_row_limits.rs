//! Tests that we can insert to tables until they reach the end row limits used in the production
//! runtime.
//!
//! This is not made as a normal unit test for two reasons.
//! 1. Since it requires loading the full proof-of-sql setups, it can take over 10 minutes to run.
//! 2. The proof-of-sql setups that the pallet uses exist in oncecells, and are shared across
//!    tests. Other tests currently use much smaller proof-of-sql setups.
#![allow(clippy::missing_docs_in_private_items)]
use clap::Parser;
use commitment_sql::OnChainTableToTableCommitmentFn;
use native_api::Api;
use on_chain_table::{OnChainColumn, OnChainTable};
use pallet_commitments::Config;
use polkadot_sdk::frame_support::assert_noop;
use polkadot_sdk::sp_runtime::BuildStorage;
use polkadot_sdk::{frame_support, frame_system, sp_io};
use proof_of_sql::base::commitment::TableCommitment;
use proof_of_sql_commitment_map::generic_over_commitment::{OptionType, TableCommitmentType};
use proof_of_sql_commitment_map::{
    CommitmentId,
    CommitmentSchemeFlags,
    GenericOverCommitmentFn,
    PerCommitmentScheme,
};
use proof_of_sql_static_setups::io::{
    initialize_from_config,
    ProofOfSqlPublicSetupArgs,
    PUBLIC_SETUPS,
};
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::Ident;
use sqlparser::dialect::PostgreSqlDialect;
use sxt_core::tables::TableIdentifier;

mod mock {
    use polkadot_sdk::frame_support::derive_impl;
    use polkadot_sdk::{frame_support, frame_system};
    use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
    use proof_of_sql_commitment_map::PerCommitmentScheme;

    type Block = frame_system::mocking::MockBlock<Test>;

    frame_support::construct_runtime!(
        pub enum Test
        {
            System: frame_system,
            CommitmentsModule: pallet_commitments,
        }
    );

    #[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
    impl frame_system::Config for Test {
        type Block = Block;
    }

    impl pallet_commitments::Config for Test {
        const END_ROW_LIMITS_PER_SCHEME: PerCommitmentScheme<ConcreteType<u32>> =
            PerCommitmentScheme {
                hyper_kzg: 268_435_455,
                dynamic_dory: 2_147_483_647,
            };
    }
}

use mock::Test;

/// Tests that we can insert to tables until they reach the end row limits used in the production
/// runtime.
#[derive(Debug, Parser)]
struct CliArgs {
    #[command(flatten)]
    setup_args: ProofOfSqlPublicSetupArgs,
}

/// Build genesis storage according to the mock runtime.
fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_commitments::GenesisConfig::<Test>::default()
        .assimilate_storage(&mut storage)
        .unwrap();

    storage.into()
}

/// Returns the table commitment with a manually written range of
/// 0..end_row.
fn commitment_with_end_row<C: CommitmentId>(
    table_commitment: TableCommitment<C>,
    end_row: usize,
) -> TableCommitment<C> {
    let column_commitments = table_commitment.column_commitments().clone();

    TableCommitment::try_new(column_commitments, 0..end_row).unwrap()
}

/// `GenericOverCommitmentFn` for [`commitment_with_end_row`].
struct CommitmentWithEndRowFn(usize);

impl GenericOverCommitmentFn for CommitmentWithEndRowFn {
    type In = TableCommitmentType;
    type Out = TableCommitmentType;

    fn call<C: CommitmentId>(&self, input: TableCommitment<C>) -> TableCommitment<C> {
        commitment_with_end_row(input, self.0)
    }
}

/// A single-row OnChainTable for the test ANIMAL.POPULATION table.
fn animal_population_data_single_row() -> OnChainTable {
    OnChainTable::try_from_iter([
        (
            Ident::new("animal"),
            OnChainColumn::VarChar(vec!["snake".to_string()]),
        ),
        (Ident::new("population"), OnChainColumn::BigInt(vec![1])),
    ])
    .unwrap()
}

/// Dummy commitments for the test ANIMAL.POPULATION table with the range 0..end_row.
fn animal_population_commitments_with_end_row(
    commitment_scheme_flags: CommitmentSchemeFlags,
    end_row: usize,
) -> PerCommitmentScheme<OptionType<TableCommitmentType>> {
    PUBLIC_SETUPS
        .get()
        .unwrap()
        .select(&commitment_scheme_flags)
        .into_flat_iter()
        .map(|any| {
            any.map(OnChainTableToTableCommitmentFn::new(
                &animal_population_data_single_row(),
                0,
            ))
            .transpose_result()
            .unwrap()
            .map(CommitmentWithEndRowFn(end_row))
        })
        .collect::<PerCommitmentScheme<OptionType<TableCommitmentType>>>()
}

/// Table identifier and create statement for the test ANIMAL.POPULATION table.
fn animal_population_table_definition() -> (TableIdentifier, CreateTableBuilder) {
    let table_id = TableIdentifier {
        namespace: b"ANIMAL".to_vec().try_into().unwrap(),
        name: b"POPULATION".to_vec().try_into().unwrap(),
    };
    let sql_statement = "CREATE TABLE animal.population (
            animal VARCHAR NOT NULL,
            population BIGINT NOT NULL,
            PRIMARY KEY (animal))"
        .to_string();
    let create_table = sqlparser::parser::Parser::new(&PostgreSqlDialect {})
        .try_with_sql(&sql_statement)
        .unwrap()
        .parse_statement()
        .unwrap()
        .try_into()
        .unwrap();

    (table_id, create_table)
}

fn we_can_reach_limit_hyper_kzg() {
    new_test_ext().execute_with(|| {
        let commitments_bytes = animal_population_commitments_with_end_row(CommitmentSchemeFlags {
            hyper_kzg: true,
            ..Default::default()
        },
        Test::END_ROW_LIMITS_PER_SCHEME.hyper_kzg as usize - 1,
        ).try_into().unwrap();

        let (table_identifier, create_table) = animal_population_table_definition();

        assert!(pallet_commitments::Pallet::<Test>::process_create_table_from_snapshot_and_initiate_commitments(create_table, commitments_bytes).is_ok());

        let insert_data = animal_population_data_single_row();

        // we can insert one more row
        assert!(pallet_commitments::Pallet::<Test>::process_insert_and_update_commitments::<Api>(table_identifier.clone(), insert_data.clone()).is_ok());

        // we cannot insert any more, and get the expected error
        assert_noop!(pallet_commitments::Pallet::<Test>::process_insert_and_update_commitments::<Api>(table_identifier, insert_data), pallet_commitments::Error::<Test>::InsertExceedsLimit);
    });
}

fn we_can_reach_limit_dynamic_dory() {
    new_test_ext().execute_with(|| {
        let commitments_bytes = animal_population_commitments_with_end_row(CommitmentSchemeFlags {
            dynamic_dory: true,
            ..Default::default()
        },
        Test::END_ROW_LIMITS_PER_SCHEME.dynamic_dory as usize - 1,
        ).try_into().unwrap();

        let (table_identifier, create_table) = animal_population_table_definition();

        assert!(pallet_commitments::Pallet::<Test>::process_create_table_from_snapshot_and_initiate_commitments(create_table, commitments_bytes).is_ok());

        let insert_data = animal_population_data_single_row();

        // we can insert one more row
        assert!(pallet_commitments::Pallet::<Test>::process_insert_and_update_commitments::<Api>(table_identifier.clone(), insert_data.clone()).is_ok());

        // we cannot insert any more, and get the expected error
        assert_noop!(pallet_commitments::Pallet::<Test>::process_insert_and_update_commitments::<Api>(table_identifier, insert_data), pallet_commitments::Error::<Test>::InsertExceedsLimit);
    });
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let CliArgs { setup_args } = CliArgs::parse();
    initialize_from_config(&setup_args).await.unwrap();

    log::info!("checking hyper_kzg limit...");
    we_can_reach_limit_hyper_kzg();
    log::info!("OK!");

    log::info!("checking dynamic_dory limit...");
    we_can_reach_limit_dynamic_dory();
    log::info!("OK!");
}
