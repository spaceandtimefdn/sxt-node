use std::fs::File;
use std::path::{Path, PathBuf};

use codec::{Decode, Encode};
use commitment_sql::proptest::table_commitment_per_commitment_scheme;
use native::interface;
use on_chain_table::proptest::{on_chain_table, proof_of_sql_schema};
use proof_of_sql_commitment_map::generic_over_commitment::AssociatedPublicSetupType;
use proof_of_sql_commitment_map::proptest::commitment_scheme_flags;
use proof_of_sql_commitment_map::{
    PerCommitmentScheme,
    TableCommitmentBytesPerCommitmentSchemePassBy,
    TableCommitmentPerCommitmentScheme,
};
use proof_of_sql_static_setups::io::{
    get_or_init_from_files_with_four_points_unchecked,
    PUBLIC_SETUPS,
};
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRng, TestRunner};
use sp_core::keccak_256;
use sxt_core::native::OnChainTableBytes;
use sxt_core::proptest::table_identifier;
use sxt_core::tables::TableIdentifier;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Case<I, O> {
    input: I,
    output: O,
}

fn write_cases<S, I, O, F, P>(strategy: S, f: F, case_directory: P)
where
    S: Strategy<Value = I>,
    F: Fn(I) -> O,
    I: Encode + Clone + std::fmt::Debug,
    O: Encode,
    P: AsRef<Path> + Clone,
{
    let config = proptest::test_runner::Config {
        cases: 64,
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Default::default()
    };

    // using a deterministic RNG essentially gives the generation some idempotency, we won't
    // clutter the cases directory with new cases unless some strategy has changed.
    let rng = TestRng::deterministic_rng(config.rng_algorithm);

    let mut runner = TestRunner::new_with_rng(config, rng);

    runner
        .run(&strategy, move |input| {
            let output = f(input.clone());

            let input_bytes = input.encode();

            let input_hash = keccak_256(&input_bytes);

            let case = Case { input, output };

            let case_bytes = case.encode();

            std::fs::write(
                case_directory.as_ref().join(hex::encode(input_hash)),
                &case_bytes,
            );

            Ok(())
        })
        .unwrap();
}

prop_compose! {
    fn table_commitment_bytes_per_commitment_scheme_pass_by(table_commitment_per_commitment_scheme: impl Strategy<Value = TableCommitmentPerCommitmentScheme>)
        (table_commitment_per_commitment_scheme in table_commitment_per_commitment_scheme)
        -> TableCommitmentBytesPerCommitmentSchemePassBy {
        let data = table_commitment_per_commitment_scheme.try_into().unwrap();

        TableCommitmentBytesPerCommitmentSchemePassBy { data }
    }
}

fn process_insert_input<'a>(
    setups: PerCommitmentScheme<AssociatedPublicSetupType<'a>>,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'a> {
    (proof_of_sql_schema(1usize..64), 0..4usize)
        .prop_flat_map(move |(schema, commitment_row_count)| {
            (
                table_identifier(),
                on_chain_table(Just(schema.clone()), 0..(4 - commitment_row_count)),
                table_commitment_per_commitment_scheme(
                    setups,
                    on_chain_table(Just(schema), Just(commitment_row_count)),
                    commitment_scheme_flags(),
                ),
            )
        })
        .prop_map(
            |(table_identifier, on_chain_table, table_commitment_per_commitment_scheme)| {
                let on_chain_table_bytes = on_chain_table.try_into().unwrap();
                let table_commitment_bytes_per_commitment_scheme_pass_by =
                    TableCommitmentBytesPerCommitmentSchemePassBy {
                        data: table_commitment_per_commitment_scheme.try_into().unwrap(),
                    };

                (
                    table_identifier,
                    on_chain_table_bytes,
                    table_commitment_bytes_per_commitment_scheme_pass_by,
                )
            },
        )
}

fn main() {
    let workspace_dir = std::env::var("CARGO_WORKSPACE_DIR").unwrap();
    write_cases(
        process_insert_input(*get_or_init_from_files_with_four_points_unchecked()),
        |(table_identifier, insert, commitments)| {
            interface::process_insert(table_identifier, insert, commitments)
        },
        format!("{workspace_dir}/native/backwards_compatibility_cases/process_insert"),
    );
}
