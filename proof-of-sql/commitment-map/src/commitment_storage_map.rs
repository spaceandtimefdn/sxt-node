//! Contains implementation of `CommitmentMap` for substrate's `StorageMap`.

use crate::{
    commitment_map_implementor::CommitmentMapImplementor,
    commitment_scheme::{AnyCommitmentScheme, CommitmentScheme},
    generic_over_commitment::ConcreteType,
};
use codec::{Decode, Encode, MaxEncodedLen};
use core::marker::PhantomData;
use frame_support::{storage::StorageDoubleMap, BoundedVec};
use proof_of_sql::base::commitment::{Commitment, TableCommitment};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use sp_core::{ConstU32, TypedGet};
use sxt_core::tables::{MaxColsPerTable, TableIdentifier};

/// Maximum byte length of a TableCommitment with 64 columns, as a constant.
const TABLE_COMMITMENT_MAX_LENGTH: u32 = 45_328;

/// Maximum byte length of a TableCommitment with 64 columns, as a type alias.
type TableCommitmentMaxLength = ConstU32<TABLE_COMMITMENT_MAX_LENGTH>;

/// Postcard-serialized TableCommitment stored in substrate [`CommitmentMap`] implementation.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct TableCommitmentBytes {
    /// Raw postcard-serialized bytes.
    data: BoundedVec<u8, TableCommitmentMaxLength>,
}

/// Errors that can occur when converting a `TableCommitment` to [`TableCommitmentBytes`].
#[derive(Debug, Snafu)]
pub enum TableCommitmentToBytesError {
    /// `TableCommitment` exceeds maximum column count.
    #[snafu(display("TableCommitment exceeds maximum column count: {num_columns}"))]
    TooManyColumns { num_columns: usize },
    /// Failed to serialize TableCommitment.
    #[snafu(display("failed to serialize TableCommitment: {error}"))]
    Postcard { error: postcard::Error },
}

impl From<postcard::Error> for TableCommitmentToBytesError {
    fn from(error: postcard::Error) -> Self {
        TableCommitmentToBytesError::Postcard { error }
    }
}

impl<C: Commitment + Serialize> TryFrom<&TableCommitment<C>> for TableCommitmentBytes {
    type Error = TableCommitmentToBytesError;

    fn try_from(value: &TableCommitment<C>) -> Result<Self, Self::Error> {
        let num_columns = value.num_columns();
        if num_columns > MaxColsPerTable::get() as usize {
            return Err(TableCommitmentToBytesError::TooManyColumns { num_columns });
        }

        let bytes = postcard::to_allocvec(&value)?;

        Ok(TableCommitmentBytes {
            data: bytes.try_into().expect("TableCommitment that doesn't exceed maximum num columns shouldn't serialize to more than TABLE_COMMITMENT_MAX_LENGTH bytes"),
        })
    }
}

impl<'de, C: Commitment + Deserialize<'de>> TryFrom<&'de TableCommitmentBytes>
    for TableCommitment<C>
{
    type Error = postcard::Error;

    fn try_from(value: &'de TableCommitmentBytes) -> Result<Self, Self::Error> {
        postcard::from_bytes(value.data.as_slice())
    }
}

/// Instantiable type leveraging a substrate [`StorageMap`] for commitments.
///
/// Implements [`CommitmentMap`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitmentStorageMapHandler<S>(PhantomData<S>)
where
    S: StorageDoubleMap<
        TableIdentifier,
        CommitmentScheme,
        TableCommitmentBytes,
        Query = Option<TableCommitmentBytes>,
    >;

impl<S> CommitmentStorageMapHandler<S>
where
    S: StorageDoubleMap<
        TableIdentifier,
        CommitmentScheme,
        TableCommitmentBytes,
        Query = Option<TableCommitmentBytes>,
    >,
{
    /// Construct a new [`CommitmentStorageMapHandler`].
    pub fn new() -> Self {
        CommitmentStorageMapHandler(PhantomData)
    }
}

impl<S> CommitmentMapImplementor<TableIdentifier, ConcreteType<TableCommitmentBytes>>
    for CommitmentStorageMapHandler<S>
where
    S: StorageDoubleMap<
        TableIdentifier,
        CommitmentScheme,
        TableCommitmentBytes,
        Query = Option<TableCommitmentBytes>,
    >,
{
    fn has_key_and_scheme_impl(&self, key: &TableIdentifier, scheme: &CommitmentScheme) -> bool {
        S::contains_key(key, scheme)
    }

    fn set_commitment_for_any_scheme_impl(
        &mut self,
        key: TableIdentifier,
        commitment: AnyCommitmentScheme<ConcreteType<TableCommitmentBytes>>,
    ) {
        let scheme = commitment.to_scheme();

        S::insert(key, scheme, commitment.unwrap());
    }

    fn delete_commitment_for_any_scheme_impl(
        &mut self,
        key: &TableIdentifier,
        scheme: &CommitmentScheme,
    ) {
        S::remove(key, scheme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, string::String, vec};
    use on_chain_table::{OnChainColumn, OnChainTable};
    use proof_of_sql::proof_primitive::dory::{
        DoryCommitment, DoryProverPublicSetup, DoryScalar, ProverSetup, PublicParameters,
    };
    use rand::{rngs::SmallRng, SeedableRng};

    #[test]
    fn we_can_deserialize_and_reserialize_dory_table_commitment_to_bytes() {
        let public_parameters = PublicParameters::rand(4, &mut SmallRng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);
        let dory_prover_setup = DoryProverPublicSetup::new(&prover_setup, 3);

        let table = OnChainTable::try_from_iter([
            (
                "animal".parse().unwrap(),
                OnChainColumn::VarChar(["cow", "cat", "dog"].map(String::from).to_vec()),
            ),
            (
                "population".parse().unwrap(),
                OnChainColumn::BigInt(vec![75, 7, 2]),
            ),
        ])
        .unwrap();

        let commitment = TableCommitment::<DoryCommitment>::try_from_columns_with_offset(
            table.iter_committable::<DoryScalar>().map(Result::unwrap),
            0,
            &dory_prover_setup,
        )
        .unwrap();

        let serialized = TableCommitmentBytes::try_from(&commitment).unwrap();

        let deserialized = TableCommitment::<DoryCommitment>::try_from(&serialized).unwrap();

        assert_eq!(deserialized, commitment);
    }

    #[test]
    fn table_commitment_max_length_is_a_reasonable_estimate() {
        let public_parameters = PublicParameters::rand(4, &mut SmallRng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);
        let dory_prover_setup = DoryProverPublicSetup::new(&prover_setup, 3);

        let column_names = (0..MaxColsPerTable::get())
            .map(|col_num| format!("col_{col_num:060}").parse().unwrap());

        let columns = (0..MaxColsPerTable::get()).map(|offset| {
            OnChainColumn::Int128(vec![i128::MAX - offset as i128, i128::MIN + offset as i128])
        });

        let table = OnChainTable::try_from_iter(column_names.zip(columns)).unwrap();

        let commitment = TableCommitment::<DoryCommitment>::try_from_columns_with_offset(
            table.iter_committable::<DoryScalar>().map(Result::unwrap),
            0,
            &dory_prover_setup,
        )
        .unwrap();

        let serialized = TableCommitmentBytes::try_from(&commitment).unwrap();

        assert!(serialized.data.len() < TABLE_COMMITMENT_MAX_LENGTH as usize);
        assert!(serialized.data.len() > ((TABLE_COMMITMENT_MAX_LENGTH as usize / 10) * 9));
    }

    #[test]
    fn we_cannot_create_bytes_from_table_commitment_with_too_many_columns() {
        let public_parameters = PublicParameters::rand(4, &mut SmallRng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);
        let dory_prover_setup = DoryProverPublicSetup::new(&prover_setup, 3);

        let column_names = (0..MaxColsPerTable::get() + 1)
            .map(|col_num| format!("col_{col_num:060}").parse().unwrap());

        let columns = core::iter::repeat(OnChainColumn::BigInt(vec![]));

        let table = OnChainTable::try_from_iter(column_names.zip(columns)).unwrap();

        let commitment = TableCommitment::<DoryCommitment>::try_from_columns_with_offset(
            table.iter_committable::<DoryScalar>().map(Result::unwrap),
            0,
            &dory_prover_setup,
        )
        .unwrap();

        assert!(matches!(
            TableCommitmentBytes::try_from(&commitment),
            Err(TableCommitmentToBytesError::TooManyColumns { .. })
        ));
    }
}
