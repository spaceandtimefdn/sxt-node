use alloc::vec;
use alloc::vec::Vec;
use core::marker::PhantomData;

use itertools::Itertools;
use on_chain_table::{OnChainTable, OutOfScalarBounds};
use proof_of_sql::base::commitment::{
    AppendColumnCommitmentsError,
    AppendTableCommitmentError,
    ColumnCommitmentsMismatch,
    Commitment,
};
use proof_of_sql_commitment_map::generic_over_commitment::{
    AssociatedPublicSetupType,
    ConcreteType,
    GenericOverCommitment,
    OptionType,
    PairType,
    ResultOkType,
    TableCommitmentType,
};
use proof_of_sql_commitment_map::{GenericOverCommitmentFn, PerCommitmentScheme};
use snafu::Snafu;
use sxt_core::tables::TableIdentifier;

use crate::row_number_column::on_chain_table_with_row_number_column;

/// Generically accepts a table commitment and returns the end of its row range.
struct GetTableCommitmentRangeEndFn;

impl GenericOverCommitmentFn for GetTableCommitmentRangeEndFn {
    type In = TableCommitmentType;
    type Out = PairType<TableCommitmentType, ConcreteType<usize>>;

    fn call<C: Commitment>(
        &self,
        input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
    ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
        let row_count = input.range().end;
        (input, row_count)
    }
}

/// Generically accepts some `T: GenericOverCommitment` and returns `Some(T)`.
struct SomeFn<T: GenericOverCommitment>(PhantomData<T>);

impl<T: GenericOverCommitment> SomeFn<T> {
    /// Construct a new [`SomeFn`].
    fn new() -> Self {
        SomeFn(PhantomData)
    }
}

impl<T: GenericOverCommitment> GenericOverCommitmentFn for SomeFn<T> {
    type In = T;
    type Out = OptionType<T>;

    fn call<C: Commitment>(
        &self,
        input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
    ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
        Some(input)
    }
}

/// Generically accepts a pair of options and returns an option of pairs.
pub struct OptionZipFn<T: GenericOverCommitment, U: GenericOverCommitment>(
    PhantomData<T>,
    PhantomData<U>,
);

impl<T: GenericOverCommitment, U: GenericOverCommitment> OptionZipFn<T, U> {
    /// Construct a new [`OptionZipFn`].
    pub fn new() -> Self {
        OptionZipFn(PhantomData, PhantomData)
    }
}

impl<T: GenericOverCommitment, U: GenericOverCommitment> GenericOverCommitmentFn
    for OptionZipFn<T, U>
{
    type In = PairType<OptionType<T>, OptionType<U>>;
    type Out = OptionType<PairType<T, U>>;

    fn call<C: Commitment>(
        &self,
        input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
    ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
        input.0.zip(input.1)
    }
}

/// Errors that can occur when appending an `OnChainTable` to a `TableCommitment`.
#[derive(Debug, Snafu)]
pub enum AppendOnChainTableError {
    /// Commitment metadata indicates that operand tables cannot be the same.
    #[snafu(transparent)]
    ColumnCommitmentsMismatch {
        /// Source column commitments mismatch error.
        source: ColumnCommitmentsMismatch,
    },
    /// Some element in the `OnChainTable` is out of bounds of target scalar field.
    #[snafu(transparent)]
    OutOfScalarBounds {
        /// Source out-of-scalar-bounds error.
        source: OutOfScalarBounds,
    },
}

struct AppendOnChainTableToTableCommitmentFn<'a, 's>(&'a OnChainTable, PhantomData<&'s ()>);

impl<'a, 's> AppendOnChainTableToTableCommitmentFn<'a, 's> {
    fn new(table: &'a OnChainTable) -> Self {
        AppendOnChainTableToTableCommitmentFn(table, PhantomData)
    }
}

impl<'a, 's> GenericOverCommitmentFn for AppendOnChainTableToTableCommitmentFn<'a, 's> {
    type In = PairType<TableCommitmentType, AssociatedPublicSetupType<'s>>;
    type Out = ResultOkType<TableCommitmentType, AppendOnChainTableError>;

    fn call<C: Commitment>(
        &self,
        input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
    ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
        let committable_table = self
            .0
            .iter_committable::<C::Scalar>()
            .collect::<Result<Vec<_>, _>>()?;

        let mut table_commitment = input.0;
        table_commitment
            .try_append_rows(committable_table, &input.1)
            .map_err(|append_error| match append_error {
                AppendTableCommitmentError::AppendColumnCommitments { source: e } => match e {
                    AppendColumnCommitmentsError::Mismatch { source: e } => e,
                    AppendColumnCommitmentsError::DuplicateIdentifiers { .. } => {
                        panic!("OnChainTables cannot have duplicate identifiers");
                    }
                },
                AppendTableCommitmentError::MixedLengthColumns { .. } => {
                    panic!("OnChainTables cannot have columns of mixed length");
                }
            })?;

        Ok(table_commitment)
    }
}

/// Errors that can occur when processing an insert to support commitment metadata.
#[derive(Debug, Snafu)]
pub enum ProcessInsertError {
    /// Unable to append table commitment.
    #[snafu(display("unable to append table commitment: {source}"), context(false))]
    AppendOnChainTable {
        /// Source append-on-chain-table error.
        source: AppendOnChainTableError,
    },
    /// Table commitments (of different schemes) have different ranges.
    #[snafu(display("table commitments have differing ranges"))]
    TableCommitmentRangeMismatch,
    /// No commitments to update.
    #[snafu(display("no commitments to update"))]
    NoCommitments,
}

/// Insert transformed to support commitment metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertAndCommitmentMetadata {
    /// The original insert with additional meta columns.
    pub insert_with_meta_columns: OnChainTable,
    /// Inserts to perform on metadata tables.
    pub meta_table_inserts: Vec<(TableIdentifier, OnChainTable)>,
}

/// Process insert to support commitment metadata.
///
/// Returns..
/// - the processed insert as [`InsertAndCommitmentMetadata`]
/// - the updated commitments for the table
pub fn process_insert(
    _table_identifier: &TableIdentifier,
    insert_data: OnChainTable,
    previous_commitments: PerCommitmentScheme<OptionType<TableCommitmentType>>,
    setups: PerCommitmentScheme<AssociatedPublicSetupType>,
) -> Result<
    (
        InsertAndCommitmentMetadata,
        PerCommitmentScheme<OptionType<TableCommitmentType>>,
    ),
    ProcessInsertError,
> {
    let (previous_commitments, row_counts): (Vec<_>, Vec<_>) = previous_commitments
        .into_flat_iter()
        .map(|any| {
            let (commitment, row_count) = any.map(GetTableCommitmentRangeEndFn).unzip();
            (commitment, row_count.unwrap())
        })
        .unzip();

    let previous_commitments = PerCommitmentScheme::from_iter(previous_commitments);

    let row_count = row_counts
        .into_iter()
        .all_equal_value()
        .map_err(|maybe_unequal| match maybe_unequal {
            Some(_) => ProcessInsertError::TableCommitmentRangeMismatch,
            None => ProcessInsertError::NoCommitments,
        })?;

    let commitments = previous_commitments
        .zip(setups.map(SomeFn::new()))
        .map(OptionZipFn::new())
        .into_flat_iter()
        .map(|any| {
            any.map(AppendOnChainTableToTableCommitmentFn::new(&insert_data))
                .transpose_result()
        })
        .collect::<Result<_, _>>()?;

    let insert_with_meta_columns = on_chain_table_with_row_number_column(insert_data, row_count);

    Ok((
        InsertAndCommitmentMetadata {
            insert_with_meta_columns,
            meta_table_inserts: vec![],
        },
        commitments,
    ))
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use on_chain_table::OnChainColumn;
    use primitive_types::U256;
    use proof_of_sql::base::commitment::TableCommitment;
    use proof_of_sql::base::database::ColumnType;
    use proof_of_sql::base::math::decimal::Precision;
    use proof_of_sql::proof_primitive::dory::{
        DoryCommitment,
        DoryProverPublicSetup,
        DoryScalar,
        ProverSetup,
        PublicParameters,
    };
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

    use super::*;

    #[test]
    fn we_can_process_inserts() {
        let public_parameters = PublicParameters::rand(4, &mut ChaCha20Rng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);
        let dory_prover_setup = DoryProverPublicSetup::new(&prover_setup, 3);

        let setups = PerCommitmentScheme::<AssociatedPublicSetupType> {
            ipa: (),
            dory: dory_prover_setup,
        };

        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let animals_col_id = "animals".parse().unwrap();
        let animals_data = ["cow", "dog", "cat"].map(String::from);

        let population_col_id = "population".parse().unwrap();
        let population_data = [100, 2, 7];

        let row_number_col_id = "META_ROW_NUMBER".parse().unwrap();
        let row_number_data = [1, 2, 3];

        let empty_table = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::empty_with_type(ColumnType::VarChar),
            ),
            (
                population_col_id,
                OnChainColumn::empty_with_type(ColumnType::BigInt),
            ),
        ])
        .unwrap();
        let empty_commitments = PerCommitmentScheme {
            ipa: None,
            dory: Some(
                TableCommitment::<DoryCommitment>::try_from_columns_with_offset(
                    empty_table
                        .iter_committable::<DoryScalar>()
                        .map(Result::unwrap),
                    0,
                    &dory_prover_setup,
                )
                .unwrap(),
            ),
        };

        let first_insert = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::VarChar(animals_data[0..2].to_vec()),
            ),
            (
                population_col_id,
                OnChainColumn::BigInt(population_data[0..2].to_vec()),
            ),
        ])
        .unwrap();

        let expected_first_insert_with_meta_columns = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::VarChar(animals_data[0..2].to_vec()),
            ),
            (
                population_col_id,
                OnChainColumn::BigInt(population_data[0..2].to_vec()),
            ),
            (
                row_number_col_id,
                OnChainColumn::BigInt(row_number_data[0..2].to_vec()),
            ),
        ])
        .unwrap();
        let expected_first_commitments = PerCommitmentScheme {
            ipa: None,
            dory: Some(
                TableCommitment::<DoryCommitment>::try_from_columns_with_offset(
                    first_insert
                        .iter_committable::<DoryScalar>()
                        .map(Result::unwrap),
                    0,
                    &dory_prover_setup,
                )
                .unwrap(),
            ),
        };

        assert_eq!(
            process_insert(&table_id, first_insert, empty_commitments, setups.clone()).unwrap(),
            (
                InsertAndCommitmentMetadata {
                    insert_with_meta_columns: expected_first_insert_with_meta_columns,
                    meta_table_inserts: vec![],
                },
                expected_first_commitments.clone()
            )
        );

        let second_insert = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::VarChar(animals_data[2..].to_vec()),
            ),
            (
                population_col_id,
                OnChainColumn::BigInt(population_data[2..].to_vec()),
            ),
        ])
        .unwrap();

        let expected_second_insert_with_meta_columns = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::VarChar(animals_data[2..].to_vec()),
            ),
            (
                population_col_id,
                OnChainColumn::BigInt(population_data[2..].to_vec()),
            ),
            (
                row_number_col_id,
                OnChainColumn::BigInt(row_number_data[2..].to_vec()),
            ),
        ])
        .unwrap();
        let expected_second_commitments = PerCommitmentScheme {
            ipa: None,
            dory: Some(
                expected_first_commitments
                    .clone()
                    .dory
                    .unwrap()
                    .try_add(
                        TableCommitment::try_from_columns_with_offset(
                            second_insert
                                .iter_committable::<DoryScalar>()
                                .map(Result::unwrap),
                            2,
                            &dory_prover_setup,
                        )
                        .unwrap(),
                    )
                    .unwrap(),
            ),
        };

        assert_eq!(
            process_insert(&table_id, second_insert, expected_first_commitments, setups).unwrap(),
            (
                InsertAndCommitmentMetadata {
                    insert_with_meta_columns: expected_second_insert_with_meta_columns,
                    meta_table_inserts: vec![],
                },
                expected_second_commitments
            )
        );
    }

    #[test]
    fn we_cannot_process_insert_with_differing_commitment_ranges() {
        let public_parameters = PublicParameters::rand(4, &mut ChaCha20Rng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);
        let dory_prover_setup = DoryProverPublicSetup::new(&prover_setup, 3);

        let setups = PerCommitmentScheme::<AssociatedPublicSetupType> {
            ipa: (),
            dory: dory_prover_setup,
        };

        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let animals_col_id = "animals".parse().unwrap();
        let animals_data = ["cow", "dog", "cat"].map(String::from);

        let population_col_id = "population".parse().unwrap();
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

        let previous_commitments = PerCommitmentScheme {
            ipa: Some(TableCommitment::try_new(Default::default(), 0..2).unwrap()),
            dory: Some(TableCommitment::try_new(Default::default(), 0..3).unwrap()),
        };

        assert!(matches!(
            process_insert(&table_id, insert_data, previous_commitments, setups),
            Err(ProcessInsertError::TableCommitmentRangeMismatch)
        ));
    }

    #[test]
    fn we_cannot_process_insert_with_mismatched_table_metadata() {
        let public_parameters = PublicParameters::rand(4, &mut ChaCha20Rng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);
        let dory_prover_setup = DoryProverPublicSetup::new(&prover_setup, 3);

        let setups = PerCommitmentScheme::<AssociatedPublicSetupType> {
            ipa: (),
            dory: dory_prover_setup,
        };

        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let animals_col_id = "animals".parse().unwrap();

        let population_col_id = "population".parse().unwrap();
        let population_data = [100, 2, 7];

        let empty_table = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::empty_with_type(ColumnType::VarChar),
            ),
            (
                population_col_id,
                OnChainColumn::empty_with_type(ColumnType::BigInt),
            ),
        ])
        .unwrap();
        let previous_commitments = PerCommitmentScheme {
            ipa: None,
            dory: Some(
                TableCommitment::<DoryCommitment>::try_from_columns_with_offset(
                    empty_table
                        .iter_committable::<DoryScalar>()
                        .map(Result::unwrap),
                    0,
                    &dory_prover_setup,
                )
                .unwrap(),
            ),
        };

        let insert_missing_column = OnChainTable::try_from_iter([(
            population_col_id,
            OnChainColumn::BigInt(population_data.to_vec()),
        )])
        .unwrap();

        assert!(matches!(
            process_insert(
                &table_id,
                insert_missing_column,
                previous_commitments,
                setups
            ),
            Err(ProcessInsertError::AppendOnChainTable {
                source: AppendOnChainTableError::ColumnCommitmentsMismatch { .. }
            })
        ));
    }

    #[test]
    fn we_cannot_process_insert_with_no_commitments() {
        let public_parameters = PublicParameters::rand(4, &mut ChaCha20Rng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);
        let dory_prover_setup = DoryProverPublicSetup::new(&prover_setup, 3);

        let setups = PerCommitmentScheme::<AssociatedPublicSetupType> {
            ipa: (),
            dory: dory_prover_setup,
        };

        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let animals_col_id = "animals".parse().unwrap();
        let animals_data = ["cow", "dog", "cat"].map(String::from);

        let population_col_id = "population".parse().unwrap();
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

        let none_previous_commitments = PerCommitmentScheme::default();

        assert!(matches!(
            process_insert(&table_id, insert_data, none_previous_commitments, setups),
            Err(ProcessInsertError::NoCommitments)
        ));
    }

    #[test]
    fn we_cannot_process_insert_with_out_of_bounds_value() {
        let public_parameters = PublicParameters::rand(4, &mut ChaCha20Rng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);
        let dory_prover_setup = DoryProverPublicSetup::new(&prover_setup, 3);

        let setups = PerCommitmentScheme::<AssociatedPublicSetupType> {
            ipa: (),
            dory: dory_prover_setup,
        };

        let table_id = TableIdentifier {
            namespace: b"animal".to_vec().try_into().unwrap(),
            name: b"population".to_vec().try_into().unwrap(),
        };

        let animals_col_id = "animals".parse().unwrap();
        let animals_data = ["water bear"].map(String::from);

        let population_col_id = "population".parse().unwrap();
        let population_data = [U256::MAX / 2];

        let insert_data = OnChainTable::try_from_iter([
            (
                animals_col_id,
                OnChainColumn::VarChar(animals_data.to_vec()),
            ),
            (
                population_col_id,
                OnChainColumn::Decimal75(Precision::new(75).unwrap(), 0, population_data.to_vec()),
            ),
        ])
        .unwrap();

        let previous_commitments = PerCommitmentScheme {
            ipa: Some(TableCommitment::try_new(Default::default(), 0..2).unwrap()),
            dory: Some(TableCommitment::try_new(Default::default(), 0..2).unwrap()),
        };

        assert!(matches!(
            process_insert(&table_id, insert_data, previous_commitments, setups),
            Err(ProcessInsertError::AppendOnChainTable {
                source: AppendOnChainTableError::OutOfScalarBounds { .. }
            })
        ));
    }
}
