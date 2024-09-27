use alloc::{string::String, vec::Vec};
use primitive_types::U256;
use proof_of_sql::base::math::decimal::Precision;
use proof_of_sql_parser::posql_time::{PoSQLTimeUnit, PoSQLTimeZone};
use serde::{Deserialize, Serialize};

/// Column data type for all types supported by sxt-node.
///
/// With the `arrow` feature, this implements conversion to/from arrow `ArrayRef`s.
///
/// Without the `std` feature, this type can be used in `no_std` environments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnChainColumn {
    /// Column of bools.
    Boolean(Vec<bool>),
    /// Column of 16-bit integerss.
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
    TimestampTZ(PoSQLTimeUnit, PoSQLTimeZone, Vec<i64>),
}

impl OnChainColumn {
    /// Returns the number of elements in this column.
    pub fn len(&self) -> usize {
        match self {
            OnChainColumn::Boolean(bools) => bools.len(),
            OnChainColumn::SmallInt(ints) => ints.len(),
            OnChainColumn::Int(ints) => ints.len(),
            OnChainColumn::BigInt(ints) => ints.len(),
            OnChainColumn::Int128(ints) => ints.len(),
            OnChainColumn::VarChar(strings) => strings.len(),
            OnChainColumn::Decimal75(.., ints) => ints.len(),
            OnChainColumn::TimestampTZ(.., ints) => ints.len(),
        }
    }

    /// Returns `true` if the column has no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn we_can_get_column_length() {
        let empty_column = OnChainColumn::Boolean(vec![]);
        assert_eq!(empty_column.len(), 0);
        assert!(empty_column.is_empty());

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
            PoSQLTimeZone::Utc,
            vec![1, 2, 3, 4, 5, 6],
        );
        assert_eq!(column.len(), 6);
    }
}
