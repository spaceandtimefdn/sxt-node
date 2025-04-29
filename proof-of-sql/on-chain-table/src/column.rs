use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use primitive_types::U256;
use proof_of_sql::base::commitment::CommittableColumn;
use proof_of_sql::base::database::ColumnType;
use proof_of_sql::base::math::decimal::Precision;
use proof_of_sql::base::posql_time::{PoSQLTimeUnit, PoSQLTimeZone};
use proof_of_sql::base::scalar::{Scalar, ScalarExt};
use serde::{Deserialize, Serialize};

use crate::u256_scalar_conversion::u256_to_scalar;
use crate::OutOfScalarBounds;

/// Column data type for all types supported by sxt-node.
///
/// With the `arrow` feature, this implements conversion to/from arrow `ArrayRef`s.
///
/// Without the `std` feature, this type can be used in `no_std` environments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnChainColumn {
    /// Column of bools.
    Boolean(Vec<bool>),
    /// Column of unsigned 8-bit integers.
    UnsignedTinyInt(Vec<u8>),
    /// Column of 8-bit integerss.
    TinyInt(Vec<i8>),
    /// Column of 16-bit integers.
    SmallInt(Vec<i16>),
    /// Column of 32-bit integerss.
    Int(Vec<i32>),
    /// Column of 64-bit integers.
    BigInt(Vec<i64>),
    /// Column of 128-bit integerss.
    ///
    /// NOTE: This variant is only included for historical reasons.
    /// In practice, [`OnChainColumn::Decimal75`] should be prefered.
    Int128(Vec<i128>),
    /// Column of strings.
    VarChar(Vec<String>),
    /// Column of decimals, all sharing a precision/scale.
    ///
    /// Note: The elements of this column are stored as an unsigned integer type.
    /// To interpret the data correctly, you must..
    /// - treat the unsigned bits as if they are two's compliment
    /// - scale the value by `10^-scale` (`scale` being the inner `i8` value)
    Decimal75(Precision, i8, Vec<U256>),
    /// Column of timestamps, all sharing a time unit/zone.
    TimestampTZ(PoSQLTimeUnit, Option<PoSQLTimeZone>, Vec<i64>),
    /// Variable length binary columns
    VarBinary(Vec<Vec<u8>>),
}

impl OnChainColumn {
    /// Returns the number of elements in this column.
    pub fn len(&self) -> usize {
        match self {
            OnChainColumn::Boolean(bools) => bools.len(),
            OnChainColumn::UnsignedTinyInt(ints) => ints.len(),
            OnChainColumn::TinyInt(ints) => ints.len(),
            OnChainColumn::SmallInt(ints) => ints.len(),
            OnChainColumn::Int(ints) => ints.len(),
            OnChainColumn::BigInt(ints) => ints.len(),
            OnChainColumn::Int128(ints) => ints.len(),
            OnChainColumn::VarChar(strings) => strings.len(),
            OnChainColumn::VarBinary(words) => words.len(),
            OnChainColumn::Decimal75(.., ints) => ints.len(),
            OnChainColumn::TimestampTZ(.., ints) => ints.len(),
        }
    }

    /// Returns `true` if the column has no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an empty column of the given proof-of-sql `ColumnType`.
    ///
    /// # Panics
    /// Panics if the `Scalar` type is requested.
    pub fn empty_with_type(column_type: ColumnType) -> OnChainColumn {
        match column_type {
            ColumnType::Boolean => OnChainColumn::Boolean(vec![]),
            ColumnType::VarChar => OnChainColumn::VarChar(vec![]),
            ColumnType::VarBinary => OnChainColumn::VarBinary(vec![]),
            ColumnType::Uint8 => OnChainColumn::UnsignedTinyInt(vec![]),
            ColumnType::TinyInt => OnChainColumn::TinyInt(vec![]),
            ColumnType::SmallInt => OnChainColumn::SmallInt(vec![]),
            ColumnType::Int => OnChainColumn::Int(vec![]),
            ColumnType::BigInt => OnChainColumn::BigInt(vec![]),
            ColumnType::Int128 => OnChainColumn::Int128(vec![]),
            ColumnType::Decimal75(precision, scale) => {
                OnChainColumn::Decimal75(precision, scale, vec![])
            }
            ColumnType::TimestampTZ(time_unit, time_zone) => {
                OnChainColumn::TimestampTZ(time_unit, Some(time_zone), vec![])
            }
            ColumnType::Scalar => unimplemented!(),
        }
    }

    /// Performs conversion to a proof-of-sql `CommittableColumn` in the scalar field `S`.
    pub fn try_to_committable_column<S: Scalar>(
        &self,
    ) -> Result<CommittableColumn, OutOfScalarBounds> {
        match &self {
            OnChainColumn::Boolean(bools) => Ok(CommittableColumn::Boolean(bools)),
            OnChainColumn::UnsignedTinyInt(ints) => Ok(CommittableColumn::Uint8(ints)),
            OnChainColumn::TinyInt(ints) => Ok(CommittableColumn::TinyInt(ints)),
            OnChainColumn::SmallInt(ints) => Ok(CommittableColumn::SmallInt(ints)),
            OnChainColumn::Int(ints) => Ok(CommittableColumn::Int(ints)),
            OnChainColumn::BigInt(ints) => Ok(CommittableColumn::BigInt(ints)),
            OnChainColumn::Int128(ints) => Ok(CommittableColumn::Int128(ints)),
            OnChainColumn::VarChar(strings) => Ok(CommittableColumn::VarChar(
                strings
                    .iter()
                    .map(Into::<S>::into)
                    .map(Into::<[u64; 4]>::into)
                    .collect(),
            )),

            OnChainColumn::Decimal75(precision, scale, ints) => Ok(CommittableColumn::Decimal75(
                *precision,
                *scale,
                ints.iter()
                    .map(|int| u256_to_scalar::<S>(int).map(Into::<[u64; 4]>::into))
                    .collect::<Result<_, _>>()?,
            )),
            OnChainColumn::TimestampTZ(time_unit, timezone, ints) => {
                Ok(CommittableColumn::TimestampTZ(
                    *time_unit,
                    timezone.unwrap_or(PoSQLTimeZone::utc()),
                    ints,
                ))
            }
            OnChainColumn::VarBinary(bytes) => Ok(CommittableColumn::VarBinary(
                bytes
                    .iter()
                    .map(|b| S::from_byte_slice_via_hash(b))
                    .map(Into::<[u64; 4]>::into)
                    .collect(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use proof_of_sql::base::database::OwnedColumn;
    use proof_of_sql::proof_primitive::dory::DoryScalar;
    use proof_of_sql::proof_primitive::hyperkzg::BNScalar;

    use super::*;

    #[test]
    fn we_can_convert_on_chain_varbinary_column_to_dory_committable_column() {
        let data = vec![vec![0u8, 1, 2, 3], vec![10, 20, 30, 40]];
        let column = OnChainColumn::VarBinary(data.clone());
        let committable = column.try_to_committable_column::<DoryScalar>().unwrap();
        let expected = CommittableColumn::VarBinary(
            data.into_iter()
                .map(|bytes| DoryScalar::from_byte_slice_via_hash(&bytes).into())
                .collect(),
        );
        assert_eq!(committable, expected);
    }

    #[test]
    fn we_can_convert_on_chain_varbinary_column_to_hyper_kzg_committable_column() {
        let data = vec![vec![5u8, 6, 7], vec![100, 101]];
        let column = OnChainColumn::VarBinary(data.clone());
        let committable = column.try_to_committable_column::<BNScalar>().unwrap();
        let expected = CommittableColumn::VarBinary(
            data.into_iter()
                .map(|bytes| BNScalar::from_byte_slice_via_hash(&bytes).into())
                .collect(),
        );
        assert_eq!(committable, expected);
    }

    #[test]
    fn we_can_get_column_length() {
        let empty_column = OnChainColumn::Boolean(vec![]);
        assert_eq!(empty_column.len(), 0);
        assert!(empty_column.is_empty());

        let column = OnChainColumn::UnsignedTinyInt(vec![0]);
        assert_eq!(column.len(), 1);
        assert!(!column.is_empty());

        let column = OnChainColumn::TinyInt(vec![0]);
        assert_eq!(column.len(), 1);
        assert!(!column.is_empty());

        let column = OnChainColumn::SmallInt(vec![1]);
        assert_eq!(column.len(), 1);
        assert!(!column.is_empty());

        let column = OnChainColumn::Int(vec![1, 2]);
        assert_eq!(column.len(), 2);

        let column = OnChainColumn::BigInt(vec![1, 2, 3]);
        assert_eq!(column.len(), 3);

        let column = OnChainColumn::VarChar(
            ["lorem", "ipsum", "dolor", "sit"]
                .map(String::from)
                .to_vec(),
        );
        assert_eq!(column.len(), 4);

        let column = OnChainColumn::Decimal75(
            Precision::new(10).unwrap(),
            0,
            [1, 2, 3, 4, 5].map(U256::from).to_vec(),
        );
        assert_eq!(column.len(), 5);

        let column = OnChainColumn::TimestampTZ(
            PoSQLTimeUnit::Second,
            Some(PoSQLTimeZone::utc()),
            vec![1, 2, 3, 4, 5, 6],
        );
        assert_eq!(column.len(), 6);
    }

    #[test]
    fn we_can_get_empty_column() {
        let empty_column = OnChainColumn::empty_with_type(ColumnType::Boolean);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::Uint8);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::TinyInt);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::SmallInt);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::Int);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::BigInt);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::Int128);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::VarChar);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::VarBinary);
        assert!(empty_column.is_empty());

        let empty_column = OnChainColumn::empty_with_type(ColumnType::TimestampTZ(
            PoSQLTimeUnit::Second,
            PoSQLTimeZone::utc(),
        ));
        assert!(empty_column.is_empty());

        let empty_column =
            OnChainColumn::empty_with_type(ColumnType::Decimal75(Precision::new(75).unwrap(), 0));
        assert!(empty_column.is_empty());
    }

    #[test]
    #[should_panic]
    fn we_cannot_get_empty_scalar_column() {
        let _should_panic = OnChainColumn::empty_with_type(ColumnType::Scalar);
    }

    fn we_can_convert_on_chain_column_to_committable_column<S: Scalar>() {
        let data = vec![true, false, true];
        let on_chain_bool_column = OnChainColumn::Boolean(data.clone());
        let owned_bool_column = OwnedColumn::<S>::Boolean(data);
        assert_eq!(
            on_chain_bool_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_bool_column)
        );

        let data = vec![10, 0, 20];
        let on_chain_uint8_column = OnChainColumn::UnsignedTinyInt(data.clone());
        let owned_uint8_column = OwnedColumn::<S>::Uint8(data);
        assert_eq!(
            on_chain_uint8_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_uint8_column)
        );

        let data = vec![-10, 0, 20];
        let on_chain_tinyint_column = OnChainColumn::TinyInt(data.clone());
        let owned_tinyint_column = OwnedColumn::<S>::TinyInt(data);
        assert_eq!(
            on_chain_tinyint_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_tinyint_column)
        );

        let data = vec![-10, 0, 20];
        let on_chain_smallint_column = OnChainColumn::SmallInt(data.clone());
        let owned_smallint_column = OwnedColumn::<S>::SmallInt(data);
        assert_eq!(
            on_chain_smallint_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_smallint_column)
        );

        let data = vec![-10, 0, 20];
        let on_chain_int_column = OnChainColumn::Int(data.clone());
        let owned_int_column = OwnedColumn::<S>::Int(data);
        assert_eq!(
            on_chain_int_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_int_column)
        );

        let data = vec![-10, 0, 20];
        let on_chain_bigint_column = OnChainColumn::BigInt(data.clone());
        let owned_bigint_column = OwnedColumn::<S>::BigInt(data);
        assert_eq!(
            on_chain_bigint_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_bigint_column)
        );

        let data = vec![-10, 0, 20];
        let on_chain_int128_column = OnChainColumn::Int128(data.clone());
        let owned_int128_column = OwnedColumn::<S>::Int128(data);
        assert_eq!(
            on_chain_int128_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_int128_column)
        );

        let data = vec![-10, 0, 20];
        let on_chain_int128_column = OnChainColumn::Int128(data.clone());
        let owned_int128_column = OwnedColumn::<S>::Int128(data);
        assert_eq!(
            on_chain_int128_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_int128_column)
        );

        let data = ["lorem", "ipsum", "dolor"].map(String::from).to_vec();
        let on_chain_varchar_column = OnChainColumn::VarChar(data.clone());
        let owned_varchar_column = OwnedColumn::<S>::VarChar(data);
        assert_eq!(
            on_chain_varchar_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_varchar_column)
        );

        let data = [b"lorem", b"ipsum", b"dolor"].map(Vec::from).to_vec();
        let on_chain_varbinary_column = OnChainColumn::VarBinary(data.clone());
        let owned_varbinary_column = OwnedColumn::<S>::VarBinary(data);
        assert_eq!(
            on_chain_varbinary_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_varbinary_column)
        );

        let on_chain_decimal_column = OnChainColumn::Decimal75(
            Precision::new(38).unwrap(),
            10,
            vec![U256::MAX, U256::zero(), U256::one()],
        );
        let owned_decimal_column = OwnedColumn::<S>::Decimal75(
            Precision::new(38).unwrap(),
            10,
            vec![-S::ONE, S::ZERO, S::ONE],
        );
        assert_eq!(
            on_chain_decimal_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_decimal_column)
        );

        let data = vec![-10, 0, 20];
        let on_chain_timestamp_column = OnChainColumn::TimestampTZ(
            PoSQLTimeUnit::Nanosecond,
            Some(PoSQLTimeZone::utc()),
            data.clone(),
        );
        let owned_timestamp_column =
            OwnedColumn::<S>::TimestampTZ(PoSQLTimeUnit::Nanosecond, PoSQLTimeZone::utc(), data);
        assert_eq!(
            on_chain_timestamp_column
                .try_to_committable_column::<S>()
                .unwrap(),
            CommittableColumn::from(&owned_timestamp_column)
        );
    }

    #[test]
    fn we_can_convert_on_chain_column_to_dory_committable_column() {
        we_can_convert_on_chain_column_to_committable_column::<DoryScalar>()
    }

    #[test]
    fn we_can_convert_on_chain_column_to_hyper_kzg_committable_column() {
        we_can_convert_on_chain_column_to_committable_column::<BNScalar>()
    }

    fn we_cannot_convert_out_of_bounds_on_chain_column_to_committable_column<S: Scalar>() {
        let on_chain_decimal_column = OnChainColumn::Decimal75(
            Precision::new(75).unwrap(),
            0,
            vec![U256::MAX, U256::MAX / 2, U256::one()],
        );

        assert!(matches!(
            on_chain_decimal_column.try_to_committable_column::<S>(),
            Err(OutOfScalarBounds)
        ));
    }

    #[test]
    fn we_cannot_convert_out_of_bounds_on_chain_column_to_dory_committable_column() {
        we_cannot_convert_out_of_bounds_on_chain_column_to_committable_column::<DoryScalar>()
    }

    #[test]
    fn we_cannot_convert_out_of_bounds_on_chain_column_to_hyper_kzg_committable_column() {
        we_cannot_convert_out_of_bounds_on_chain_column_to_committable_column::<BNScalar>()
    }
}
