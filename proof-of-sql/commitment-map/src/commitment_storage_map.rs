//! Contains implementation of `CommitmentMap` for substrate's `StorageMap`.

use core::marker::PhantomData;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::storage::StorageDoubleMap;
use frame_support::BoundedVec;
use proof_of_sql::base::commitment::{Commitment, TableCommitment};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use sp_core::{ConstU32, RuntimeDebug, TypedGet};
use sp_runtime_interface::pass_by::PassByCodec;
use sxt_core::native::NativeCommitmentError;
use sxt_core::tables::{MaxColsPerTable, TableIdentifier};

use crate::commitment_map_implementor::CommitmentMapImplementor;
use crate::commitment_scheme::{AnyCommitmentScheme, CommitmentScheme};
use crate::generic_over_commitment::{ConcreteType, OptionType, TableCommitmentType};
use crate::PerCommitmentScheme;

/// Maximum byte length of a TableCommitment with 64 columns, as a constant.
const TABLE_COMMITMENT_MAX_LENGTH: u32 = 45_328;

/// Maximum byte length of a TableCommitment with 64 columns, as a type alias.
pub type TableCommitmentMaxLength = ConstU32<TABLE_COMMITMENT_MAX_LENGTH>;

/// Bincode-serialized TableCommitment stored in substrate [`CommitmentMap`] implementation.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo, Serialize, Deserialize,
)]
pub struct TableCommitmentBytes {
    /// Raw bincode-serialized bytes.
    pub data: BoundedVec<u8, TableCommitmentMaxLength>,
}

/// Collection of serialized table commitments with at most one per commitment scheme.
pub type TableCommitmentBytesPerCommitmentScheme =
    PerCommitmentScheme<OptionType<ConcreteType<TableCommitmentBytes>>>;

/// [`TableCommitmentBytesPerCommitmentScheme`] wrapper that can cross the native-runtime boundary.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, PassByCodec)]
pub struct TableCommitmentBytesPerCommitmentSchemePassBy {
    /// Internal serialized table commitments.
    pub data: TableCommitmentBytesPerCommitmentScheme,
}

/// Errors that can occur when converting a `TableCommitment` to [`TableCommitmentBytes`].
#[derive(Debug, Snafu)]
pub enum TableCommitmentToBytesError {
    /// `TableCommitment` exceeds maximum column count.
    #[snafu(display("TableCommitment exceeds maximum column count: {num_columns}"))]
    TooManyColumns {
        /// The excessive number of columns
        num_columns: usize,
    },
    /// Failed to serialize TableCommitment.
    #[snafu(display("failed to serialize TableCommitment: {error}"))]
    Bincode {
        /// Source bincode error.
        error: bincode::error::EncodeError,
    },
}

impl From<bincode::error::EncodeError> for TableCommitmentToBytesError {
    fn from(error: bincode::error::EncodeError) -> Self {
        TableCommitmentToBytesError::Bincode { error }
    }
}

impl From<TableCommitmentToBytesError> for NativeCommitmentError {
    fn from(_: TableCommitmentToBytesError) -> Self {
        NativeCommitmentError::CommitmentSerialization
    }
}

impl<C: Commitment + Serialize> TryFrom<&TableCommitment<C>> for TableCommitmentBytes {
    type Error = TableCommitmentToBytesError;

    fn try_from(value: &TableCommitment<C>) -> Result<Self, Self::Error> {
        let num_columns = value.num_columns();
        if num_columns > MaxColsPerTable::get() as usize {
            return Err(TableCommitmentToBytesError::TooManyColumns { num_columns });
        }

        let bytes = bincode::serde::encode_to_vec(
            value,
            bincode::config::legacy()
                .with_fixed_int_encoding()
                .with_big_endian(),
        )?;

        Ok(TableCommitmentBytes {
            data: bytes.try_into().expect("TableCommitment that doesn't exceed maximum num columns shouldn't serialize to more than TABLE_COMMITMENT_MAX_LENGTH bytes"),
        })
    }
}

// This conversion cannot be implemented with `GenericOverCommitmentFn` because it imposes
// additional trait bounds on `WithCommitment<C>` (`C: Serialize`).
impl TryFrom<PerCommitmentScheme<OptionType<TableCommitmentType>>>
    for TableCommitmentBytesPerCommitmentScheme
{
    type Error = TableCommitmentToBytesError;

    fn try_from(
        value: PerCommitmentScheme<OptionType<TableCommitmentType>>,
    ) -> Result<Self, Self::Error> {
        value
            .into_flat_iter()
            .map(|any| match &any {
                AnyCommitmentScheme::HyperKzg(commitment) => {
                    commitment.try_into().map(AnyCommitmentScheme::HyperKzg)
                }
                AnyCommitmentScheme::DynamicDory(commitment) => {
                    commitment.try_into().map(AnyCommitmentScheme::DynamicDory)
                }
            })
            .collect()
    }
}

impl<C: Commitment> TryFrom<&TableCommitmentBytes> for TableCommitment<C>
where
    C: Commitment + for<'de> Deserialize<'de>,
{
    type Error = bincode::error::DecodeError;

    fn try_from(value: &TableCommitmentBytes) -> Result<Self, Self::Error> {
        let (commitment, _) = bincode::serde::decode_from_slice(
            value.data.as_slice(),
            bincode::config::legacy()
                .with_fixed_int_encoding()
                .with_big_endian(),
        )?;
        Ok(commitment)
    }
}

// This conversion cannot be implemented with `GenericOverCommitmentFn` because it imposes
// additional trait bounds on `WithCommitment<C>` (`C: Deserialize`).
impl TryFrom<TableCommitmentBytesPerCommitmentScheme>
    for PerCommitmentScheme<OptionType<TableCommitmentType>>
{
    type Error = bincode::error::DecodeError;

    fn try_from(value: TableCommitmentBytesPerCommitmentScheme) -> Result<Self, Self::Error> {
        value
            .into_flat_iter()
            .map(|any| match &any {
                AnyCommitmentScheme::HyperKzg(commitment) => {
                    commitment.try_into().map(AnyCommitmentScheme::HyperKzg)
                }
                AnyCommitmentScheme::DynamicDory(commitment) => {
                    commitment.try_into().map(AnyCommitmentScheme::DynamicDory)
                }
            })
            .collect()
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

    fn get_commitment_for_any_scheme_impl(
        &self,
        key: &TableIdentifier,
        scheme: &CommitmentScheme,
    ) -> AnyCommitmentScheme<OptionType<ConcreteType<TableCommitmentBytes>>> {
        match scheme {
            CommitmentScheme::HyperKzg => AnyCommitmentScheme::HyperKzg(S::get(key, scheme)),
            CommitmentScheme::DynamicDory => AnyCommitmentScheme::DynamicDory(S::get(key, scheme)),
        }
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
    use alloc::string::String;
    use alloc::{format, vec};

    use on_chain_table::{OnChainColumn, OnChainTable};
    use proof_of_sql::proof_primitive::dory::{
        DoryScalar,
        DynamicDoryCommitment,
        ProverSetup,
        PublicParameters,
    };
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;
    use sqlparser::ast::Ident;

    use super::*;

    #[test]
    fn we_can_deserialize_and_reserialize_dory_table_commitment_to_bytes() {
        let public_parameters = PublicParameters::rand(4, &mut ChaCha20Rng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);

        let table = OnChainTable::try_from_iter([
            (
                Ident::new("animal"),
                OnChainColumn::VarChar(["cow", "cat", "dog"].map(String::from).to_vec()),
            ),
            (
                Ident::new("population"),
                OnChainColumn::BigInt(vec![75, 7, 2]),
            ),
        ])
        .unwrap();

        let commitment = TableCommitment::<DynamicDoryCommitment>::try_from_columns_with_offset(
            table.iter_committable::<DoryScalar>().map(Result::unwrap),
            0,
            &&prover_setup,
        )
        .unwrap();

        let serialized = TableCommitmentBytes::try_from(&commitment).unwrap();

        let deserialized = TableCommitment::<DynamicDoryCommitment>::try_from(&serialized).unwrap();

        assert_eq!(deserialized, commitment);

        let per_commitment_scheme = PerCommitmentScheme::<OptionType<TableCommitmentType>> {
            hyper_kzg: None,
            dynamic_dory: Some(commitment),
        };

        let serialized =
            TableCommitmentBytesPerCommitmentScheme::try_from(per_commitment_scheme.clone())
                .unwrap();

        let deserialized =
            PerCommitmentScheme::<OptionType<TableCommitmentType>>::try_from(serialized).unwrap();

        assert_eq!(deserialized, per_commitment_scheme);
    }

    #[test]
    fn table_commitment_max_length_is_a_reasonable_estimate() {
        let public_parameters = PublicParameters::rand(4, &mut ChaCha20Rng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);

        let column_names =
            (0..MaxColsPerTable::get()).map(|col_num| Ident::new(format!("col_{col_num:060}")));

        let columns = (0..MaxColsPerTable::get()).map(|offset| {
            OnChainColumn::Int128(vec![i128::MAX - offset as i128, i128::MIN + offset as i128])
        });

        let table = OnChainTable::try_from_iter(column_names.zip(columns)).unwrap();

        let commitment = TableCommitment::<DynamicDoryCommitment>::try_from_columns_with_offset(
            table.iter_committable::<DoryScalar>().map(Result::unwrap),
            0,
            &&prover_setup,
        )
        .unwrap();

        let serialized = TableCommitmentBytes::try_from(&commitment).unwrap();

        assert!(serialized.data.len() < TABLE_COMMITMENT_MAX_LENGTH as usize);
        assert!(serialized.data.len() > ((TABLE_COMMITMENT_MAX_LENGTH as usize / 10) * 9));
    }

    #[test]
    fn we_cannot_create_bytes_from_table_commitment_with_too_many_columns() {
        let public_parameters = PublicParameters::rand(4, &mut ChaCha20Rng::seed_from_u64(123));
        let prover_setup = ProverSetup::from(&public_parameters);

        let column_names =
            (0..MaxColsPerTable::get() + 1).map(|col_num| Ident::new(format!("col_{col_num:060}")));

        let columns = core::iter::repeat(OnChainColumn::BigInt(vec![]));

        let table = OnChainTable::try_from_iter(column_names.zip(columns)).unwrap();

        let commitment = TableCommitment::<DynamicDoryCommitment>::try_from_columns_with_offset(
            table.iter_committable::<DoryScalar>().map(Result::unwrap),
            0,
            &&prover_setup,
        )
        .unwrap();

        assert!(matches!(
            TableCommitmentBytes::try_from(&commitment),
            Err(TableCommitmentToBytesError::TooManyColumns { .. })
        ));

        let per_commitment_scheme = PerCommitmentScheme::<OptionType<TableCommitmentType>> {
            hyper_kzg: None,
            dynamic_dory: Some(commitment),
        };

        assert!(matches!(
            TableCommitmentBytesPerCommitmentScheme::try_from(per_commitment_scheme),
            Err(TableCommitmentToBytesError::TooManyColumns { .. })
        ));
    }
}
