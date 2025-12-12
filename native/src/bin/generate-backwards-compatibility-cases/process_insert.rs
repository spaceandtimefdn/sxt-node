use std::path::Path;

use codec::{Decode, Encode};
use commitment_sql::proptest::table_commitment_per_commitment_scheme;
use on_chain_table::proptest::{
    decimal_75_column,
    decimal_75_column_type,
    i256,
    ident,
    on_chain_table,
    proof_of_sql_schema,
    ProofOfSqlSchema,
};
use on_chain_table::{OnChainTable, StringToScalarConversion};
use proof_of_sql::base::math::decimal::Precision;
use proof_of_sql_commitment_map::generic_over_commitment::AssociatedPublicSetupType;
use proof_of_sql_commitment_map::proptest::commitment_scheme_flags;
use proof_of_sql_commitment_map::{
    CommitmentSchemeFlags,
    PerCommitmentScheme,
    TableCommitmentBytesPerCommitmentSchemePassBy,
    TableCommitmentPerCommitmentScheme,
};
use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
use proptest::prelude::*;
use sxt_core::native::{NativeCommitmentError, OnChainTableBytes};
use sxt_core::proptest::table_identifier;
use sxt_core::tables::TableIdentifier;

pub use crate::write_cases;

/// Returns `process_insert` input types given their logical equivalents.
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

/// Strategy for generating happy-path input for `process_insert`.
fn process_insert_input(
    setups: PerCommitmentScheme<AssociatedPublicSetupType<'_>>,
    string_to_scalar: StringToScalarConversion,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'_> {
    (proof_of_sql_schema(1usize..64), 0..4usize)
        .prop_flat_map(move |(schema, commitment_row_count)| {
            (
                table_identifier(),
                on_chain_table(Just(schema.clone()), 0..(4 - commitment_row_count)),
                table_commitment_per_commitment_scheme(
                    setups,
                    on_chain_table(Just(schema), Just(commitment_row_count)),
                    commitment_scheme_flags(),
                    string_to_scalar,
                ),
            )
        })
        .prop_map(process_insert_logical_to_passby_input)
}

/// Strategy for generating `process_insert` input with malformed commitments.
fn process_insert_input_bad_commitments(
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
    string_to_scalar: StringToScalarConversion,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'_> {
    process_insert_input(setups, string_to_scalar).prop_perturb(
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

/// Strategy for generating `process_insert` input with malformed insert data.
fn process_insert_input_bad_table(
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
    string_to_scalar: StringToScalarConversion,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'_> {
    process_insert_input(setups, string_to_scalar).prop_perturb(
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

/// Strategy for generating `process_insert` input with out-of-bounds decimal insert data.
fn process_insert_input_out_of_scalar_bounds(
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
    string_to_scalar: StringToScalarConversion,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'_> {
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
                    string_to_scalar,
                ),
            )
        })
        .prop_map(process_insert_logical_to_passby_input)
}

/// Strategy for generating `process_insert` input with insert data/commitments that disagree on
/// table schema.
fn process_insert_input_mismatched_schemas(
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
    string_to_scalar: StringToScalarConversion,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'_> {
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
                    string_to_scalar,
                ),
            )
        })
        .prop_map(process_insert_logical_to_passby_input)
}

/// Strategy for generating `process_insert` input with commitments that disagree on table size.
fn process_insert_input_mismatched_lengths(
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
    string_to_scalar: StringToScalarConversion,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'_> {
    (proof_of_sql_schema(1..64usize), 0..4usize, 0..4usize)
        .prop_filter(
            "lengths match",
            |(_, hyperkzg_length, dynamic_dory_length)| hyperkzg_length != dynamic_dory_length,
        )
        .prop_flat_map(move |(schema, hyperkzg_length, dynamic_dory_length)| {
            (
                table_identifier(),
                on_chain_table(Just(schema.clone()), 0..(4 - hyperkzg_length)),
                (
                    table_commitment_per_commitment_scheme(
                        setups,
                        on_chain_table(Just(schema.clone()), Just(hyperkzg_length)),
                        Just(CommitmentSchemeFlags::all()),
                        string_to_scalar,
                    ),
                    table_commitment_per_commitment_scheme(
                        setups,
                        on_chain_table(Just(schema), Just(dynamic_dory_length)),
                        Just(CommitmentSchemeFlags::all()),
                        string_to_scalar,
                    ),
                )
                    .prop_map(
                        |(hyperkzg_commitments, dynamic_dory_commitments)| PerCommitmentScheme {
                            hyper_kzg: hyperkzg_commitments.hyper_kzg,
                            dynamic_dory: dynamic_dory_commitments.dynamic_dory,
                        },
                    ),
            )
        })
        .prop_map(process_insert_logical_to_passby_input)
}

/// Strategy for generating `process_insert` input with commitments that disagree on column order.
fn process_insert_input_mismatched_column_order(
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
    string_to_scalar: StringToScalarConversion,
) -> impl Strategy<
    Value = (
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ),
> + use<'_> {
    (
        proof_of_sql_schema(2..64usize)
            .prop_flat_map(|schema| {
                (
                    Just(schema.clone()),
                    Just(schema.into_vec())
                        .prop_shuffle()
                        .prop_map(|shuffled| ProofOfSqlSchema::try_from_iter(shuffled).unwrap()),
                )
            })
            .prop_filter("schema orders match", |(schema_a, schema_b)| {
                schema_a != schema_b
            }),
        0..4usize,
    )
        .prop_flat_map(move |((schema_a, schema_b), commitment_row_count)| {
            (
                table_identifier(),
                on_chain_table(Just(schema_a.clone()), 0..(4 - commitment_row_count)),
                (
                    table_commitment_per_commitment_scheme(
                        setups,
                        on_chain_table(Just(schema_a), Just(commitment_row_count)),
                        Just(CommitmentSchemeFlags::all()),
                        string_to_scalar,
                    ),
                    table_commitment_per_commitment_scheme(
                        setups,
                        on_chain_table(Just(schema_b), Just(commitment_row_count)),
                        Just(CommitmentSchemeFlags::all()),
                        string_to_scalar,
                    ),
                )
                    .prop_map(
                        |(hyperkzg_commitments, dynamic_dory_commitments)| PerCommitmentScheme {
                            hyper_kzg: hyperkzg_commitments.hyper_kzg,
                            dynamic_dory: dynamic_dory_commitments.dynamic_dory,
                        },
                    ),
            )
        })
        .prop_map(process_insert_logical_to_passby_input)
}

/// Strategy for generating `process_insert` input with no commitments.
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

fn apply_triple_fn<A, B, C, O>(f: impl Fn(A, B, C) -> O) -> impl Fn((A, B, C)) -> O {
    move |(a, b, c)| f(a, b, c)
}

/// Generates and writes cases for `process_insert_fn`.
pub fn write_process_insert_cases(
    process_insert_dir: impl AsRef<Path>,
    process_insert_fn: impl Fn(
        TableIdentifier,
        OnChainTableBytes,
        TableCommitmentBytesPerCommitmentSchemePassBy,
    ) -> Result<
        (
            OnChainTableBytes,
            TableCommitmentBytesPerCommitmentSchemePassBy,
        ),
        NativeCommitmentError,
    >,
    string_to_scalar: StringToScalarConversion,
) {
    let setups = get_or_init_from_files_with_four_points_unchecked();
    let process_insert_fn = apply_triple_fn(process_insert_fn);

    // happy path
    write_cases(
        process_insert_input(*setups, string_to_scalar),
        &process_insert_fn,
        |_, o| o.is_ok(),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_bad_commitments(*setups, string_to_scalar),
        &process_insert_fn,
        |_, o| o == &Err(NativeCommitmentError::CommitmentDeserialization),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_bad_table(*setups, string_to_scalar),
        &process_insert_fn,
        |_, o| o == &Err(NativeCommitmentError::TableDeserialization),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_out_of_scalar_bounds(*setups, string_to_scalar),
        &process_insert_fn,
        |_, o| o == &Err(NativeCommitmentError::OutOfScalarBounds),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_mismatched_lengths(*setups, string_to_scalar),
        &process_insert_fn,
        |_, o| o == &Err(NativeCommitmentError::TableCommitmentRangeMismatch),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_mismatched_column_order(*setups, string_to_scalar),
        &process_insert_fn,
        |_, o| o == &Err(NativeCommitmentError::TableCommitmentColumnOrderMismatch),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_mismatched_schemas(*setups, string_to_scalar),
        &process_insert_fn,
        |_, o| o == &Err(NativeCommitmentError::ColumnCommitmentsMismatch),
        &process_insert_dir,
    );

    write_cases(
        process_insert_input_no_commitments(),
        &process_insert_fn,
        |_, o| o == &Err(NativeCommitmentError::NoCommitments),
        &process_insert_dir,
    );
}

pub fn write_process_insert_cases_version_1(cases_dir: impl AsRef<Path>) {
    let process_insert_dir = cases_dir.as_ref().join("process_insert");

    write_process_insert_cases(
        process_insert_dir,
        native::interface::process_insert,
        StringToScalarConversion::Posql99,
    );
}

pub fn write_process_insert_cases_version_2(cases_dir: impl AsRef<Path>) {
    let process_insert_dir = cases_dir.as_ref().join("process_insert_version_2");

    write_process_insert_cases(
        process_insert_dir,
        native::interface::process_insert_version_2,
        StringToScalarConversion::Core,
    );
}
