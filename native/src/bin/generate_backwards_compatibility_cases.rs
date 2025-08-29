use std::path::Path;

use codec::{Decode, Encode};
use commitment_sql::proptest::table_commitment_per_commitment_scheme;
use native::interface;
use on_chain_table::proptest::{
    decimal_75_column,
    decimal_75_column_type,
    i256,
    ident,
    on_chain_table,
    proof_of_sql_schema,
    ProofOfSqlSchema,
};
use on_chain_table::OnChainTable;
use proof_of_sql::base::math::decimal::Precision;
use proof_of_sql_commitment_map::generic_over_commitment::AssociatedPublicSetupType;
use proof_of_sql_commitment_map::proptest::commitment_scheme_flags;
use proof_of_sql_commitment_map::{
    PerCommitmentScheme,
    TableCommitmentBytesPerCommitmentSchemePassBy,
    TableCommitmentPerCommitmentScheme,
};
use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRng, TestRunner};
use sp_core::keccak_256;
use sxt_core::native::{NativeCommitmentError, OnChainTableBytes};
use sxt_core::proptest::table_identifier;
use sxt_core::tables::TableIdentifier;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Case<I, O> {
    input: I,
    output: O,
}

fn write_cases<S, I, O, F, A, P>(strategy: S, f: F, assert: A, case_directory: P)
where
    S: Strategy<Value = I>,
    F: Fn(I) -> O,
    A: Fn(&I, &O) -> bool,
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
            let input_bytes = input.encode();

            let input_hash = keccak_256(&input_bytes);

            let case_file = case_directory.as_ref().join(hex::encode(input_hash));

            // skip cases we've already generated
            if std::fs::exists(&case_file).unwrap() {
                return Ok(());
            }

            let output = f(input.clone());

            assert!(assert(&input, &output));

            let case = Case { input, output };

            let case_bytes = case.encode();

            std::fs::write(&case_file, &case_bytes).unwrap();

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

fn process_insert_logical_to_passby_input(
    (table_identifier, on_chain_table, table_commitment_per_commitment_scheme): (
        TableIdentifier,
        OnChainTable,
        TableCommitmentPerCommitmentScheme,
    ),
) -> (
    TableIdentifier,
    OnChainTableBytes,
    TableCommitmentBytesPerCommitmentSchemePassBy,
) {
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
        .prop_map(process_insert_logical_to_passby_input)
}

fn process_insert_input_bad_commitments<'a>(
    setups: PerCommitmentScheme<AssociatedPublicSetupType<'a>>,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'a> {
    process_insert_input(setups).prop_perturb(
        |(table_identifier, on_chain_table, table_commitment_per_commitment_scheme), mut rng| {
            let data = table_commitment_per_commitment_scheme
                .data
                .into_flat_iter()
                .map(|any| {
                    let scheme = any.to_scheme();
                    let mut bytes = any.unwrap();

                    // delete a random byte from an otherwise correct commitment
                    bytes.data.remove(rng.random_range(0..bytes.data.len()));

                    scheme.into_any_concrete(bytes)
                })
                .collect();

            (
                table_identifier,
                on_chain_table,
                TableCommitmentBytesPerCommitmentSchemePassBy { data },
            )
        },
    )
}

fn process_insert_input_bad_table<'a>(
    setups: PerCommitmentScheme<AssociatedPublicSetupType<'a>>,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'a> {
    process_insert_input(setups).prop_perturb(
        |(table_identifier, on_chain_table, table_commitment_per_commitment_scheme), mut rng| {
            let mut on_chain_table_data = on_chain_table.data().clone();

            // remove a couple bytes from an otherwise correct on chain table
            on_chain_table_data.remove(rng.random_range(0..on_chain_table_data.len()));
            on_chain_table_data.remove(rng.random_range(0..on_chain_table_data.len()));

            let encoded_on_chain_table_bytes = on_chain_table_data.encode();

            let on_chain_table_bytes =
                OnChainTableBytes::decode(&mut encoded_on_chain_table_bytes.as_slice()).unwrap();

            (
                table_identifier,
                on_chain_table_bytes,
                table_commitment_per_commitment_scheme,
            )
        },
    )
}

fn process_insert_input_out_of_scalar_bounds<'a>(
    setups: PerCommitmentScheme<AssociatedPublicSetupType<'a>>,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'a> {
    (ident(), decimal_75_column_type())
        .prop_flat_map(move |(column_name, column_type)| {
            let column_name_clone = column_name.clone();
            (
                table_identifier(),
                decimal_75_column(
                    Precision::new(column_type.precision_value().unwrap()).unwrap(),
                    column_type.scale().unwrap(),
                    i256(),
                    16..64usize,
                )
                .prop_map(move |col| {
                    OnChainTable::try_from_iter([(column_name.clone(), col)]).unwrap()
                }),
                table_commitment_per_commitment_scheme(
                    setups,
                    on_chain_table(
                        Just(
                            ProofOfSqlSchema::try_from_iter([(column_name_clone, column_type)])
                                .unwrap(),
                        ),
                        0..4usize,
                    ),
                    commitment_scheme_flags(),
                ),
            )
        })
        .prop_map(process_insert_logical_to_passby_input)
}

fn process_insert_input_mismatched_schemas<'a>(
    setups: PerCommitmentScheme<AssociatedPublicSetupType<'a>>,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'a> {
    (
        proof_of_sql_schema(1..64usize),
        proof_of_sql_schema(1..64usize),
        0..4usize,
    )
        .prop_filter("schemas match", |(schema_a, schema_b, ..)| {
            schema_a != schema_b
        })
        .prop_flat_map(move |(schema_a, schema_b, commitment_row_count)| {
            (
                table_identifier(),
                on_chain_table(Just(schema_a), 0..(4 - commitment_row_count)),
                table_commitment_per_commitment_scheme(
                    setups,
                    on_chain_table(Just(schema_b), Just(commitment_row_count)),
                    commitment_scheme_flags(),
                ),
            )
        })
        .prop_map(process_insert_logical_to_passby_input)
}

fn process_insert_input_no_commitments<'a>() -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'a> {
    (
        table_identifier(),
        on_chain_table(proof_of_sql_schema(1..64usize), 0..16usize),
    )
        .prop_map(|(table_identifier, on_chain_table)| {
            let on_chain_table_bytes = on_chain_table.try_into().unwrap();
            let table_commitment_bytes_per_commitment_scheme_pass_by =
                TableCommitmentBytesPerCommitmentSchemePassBy {
                    data: None.into_iter().collect(),
                };

            (
                table_identifier,
                on_chain_table_bytes,
                table_commitment_bytes_per_commitment_scheme_pass_by,
            )
        })
}

fn process_insert_tuple(
    (table_identifier, insert, commitments): (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
) -> Result<
    (
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
    NativeCommitmentError,
> {
    interface::process_insert(table_identifier, insert, commitments)
}

fn write_process_insert_cases(cases_dir: impl AsRef<Path>) {
    let process_insert_dir = cases_dir.as_ref().join("process_insert");

    let setups = get_or_init_from_files_with_four_points_unchecked();

    // happy path
    write_cases(
        process_insert_input(*setups),
        process_insert_tuple,
        |_, o| o.is_ok(),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_bad_commitments(*setups),
        process_insert_tuple,
        |_, o| o == &Err(NativeCommitmentError::CommitmentDeserialization),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_bad_table(*setups),
        process_insert_tuple,
        |_, o| o == &Err(NativeCommitmentError::TableDeserialization),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_out_of_scalar_bounds(*setups),
        process_insert_tuple,
        |_, o| o == &Err(NativeCommitmentError::OutOfScalarBounds),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_mismatched_schemas(*setups),
        process_insert_tuple,
        |_, o| o == &Err(NativeCommitmentError::ColumnCommitmentsMismatch),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_no_commitments(),
        process_insert_tuple,
        |_, o| o == &Err(NativeCommitmentError::NoCommitments),
        &process_insert_dir,
    );
}

fn main() {
    let workspace_dir = std::env::var("CARGO_WORKSPACE_DIR").unwrap();
    let cases_dir = format!("{workspace_dir}/native/backwards_compatibility_cases/");

    write_process_insert_cases(cases_dir);
}
