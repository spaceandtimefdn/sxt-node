#[cfg(feature = "std")]
use alloc::sync::Arc;

#[cfg(feature = "std")]
use arrow::array::{ArrayRef, Decimal256Array};
#[cfg(feature = "std")]
use arrow::datatypes::{i256, DataType as ArrowDataType};
use sqlparser::ast::{ColumnDef, DataType as SqlparserDataType, ExactNumberInfo};

/// Maximum decimal precision supported by proof of sql.
pub const MAX_PRECISION: u8 = 75;

/// Returns the provided number info with precision clamped to the proof of sql maximum.
fn number_info_clamp_precision(number_info: ExactNumberInfo) -> ExactNumberInfo {
    match number_info {
        ExactNumberInfo::None => ExactNumberInfo::Precision(MAX_PRECISION as u64),
        ExactNumberInfo::Precision(p) => ExactNumberInfo::Precision(p.min(MAX_PRECISION as u64)),
        ExactNumberInfo::PrecisionAndScale(p, s) => {
            ExactNumberInfo::PrecisionAndScale(p.min(MAX_PRECISION as u64), s)
        }
    }
}

/// Returns the provided column def with precision clamped to the proof of sql maximum if the
/// column def is a decimal.
pub fn column_def_clamp_precision(column: ColumnDef) -> ColumnDef {
    let data_type = match column.data_type {
        SqlparserDataType::Numeric(number_info) => {
            SqlparserDataType::Numeric(number_info_clamp_precision(number_info))
        }
        SqlparserDataType::Decimal(number_info) => {
            SqlparserDataType::Decimal(number_info_clamp_precision(number_info))
        }
        SqlparserDataType::BigNumeric(number_info) => {
            SqlparserDataType::BigNumeric(number_info_clamp_precision(number_info))
        }
        SqlparserDataType::BigDecimal(number_info) => {
            SqlparserDataType::BigDecimal(number_info_clamp_precision(number_info))
        }
        SqlparserDataType::Dec(number_info) => {
            SqlparserDataType::Dec(number_info_clamp_precision(number_info))
        }
        data_type => data_type,
    };

    ColumnDef {
        data_type,
        ..column
    }
}

/// Returns the provided column with precision clamped to the proof of sql maximum if the column
/// is Decimal256.
#[cfg(feature = "std")]
pub fn column_clamp_precision(column: ArrayRef) -> ArrayRef {
    match column.data_type() {
        ArrowDataType::Decimal256(precision, scale) if precision > &MAX_PRECISION => {
            Arc::new(Decimal256Array::from_iter(
                column
                    .as_any()
                    .downcast_ref::<Decimal256Array>()
                    .unwrap()
                    .iter()
                    .map(|maybe_int| {
                        maybe_int.map(|int| {
                            let string_representation = int.to_string();

                            let truncated_string_representation: String = if int.is_negative() {
                                let actual_precision = string_representation.len() - 1;
                                std::iter::once('-').chain(string_representation.chars().skip(1).skip((actual_precision as i8 - MAX_PRECISION as i8).max(0) as usize)).collect()
                            } else {
                                let actual_precision = string_representation.len();
                                string_representation.chars().skip((actual_precision as i8 - MAX_PRECISION as i8).max(0) as usize).collect()
                            };

                            i256::from_string(&truncated_string_representation).expect("previous string representation minus one digit should still be valid")
                        })
                    }),
            ).with_precision_and_scale(MAX_PRECISION, *scale)
                .expect("this error is exceedingly unlikely, only occurs if the scale of the source column is 76"))
        }
        _ => column,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use sqlparser::ast::Ident;

    use super::*;

    #[test]
    fn we_can_clamp_decimal_column_def() {
        let nullable_column = ColumnDef {
            name: Ident::new("numeric_col"),
            data_type: sqlparser::ast::DataType::Numeric(ExactNumberInfo::None),
            collation: None,
            options: vec![],
        };
        let expected = ColumnDef {
            name: Ident::new("numeric_col"),
            data_type: sqlparser::ast::DataType::Numeric(ExactNumberInfo::Precision(75)),
            collation: None,
            options: vec![],
        };

        assert_eq!(&column_def_clamp_precision(nullable_column), &expected);
        assert_eq!(&column_def_clamp_precision(expected.clone()), &expected);

        let nullable_column = ColumnDef {
            name: Ident::new("dec_col"),
            data_type: sqlparser::ast::DataType::Dec(ExactNumberInfo::Precision(78)),
            collation: None,
            options: vec![],
        };

        let expected = ColumnDef {
            name: Ident::new("dec_col"),
            data_type: sqlparser::ast::DataType::Dec(ExactNumberInfo::Precision(75)),
            collation: None,
            options: vec![],
        };

        assert_eq!(&column_def_clamp_precision(nullable_column), &expected);
        assert_eq!(&column_def_clamp_precision(expected.clone()), &expected);

        let nullable_column = ColumnDef {
            name: Ident::new("decimal_col"),
            data_type: sqlparser::ast::DataType::Decimal(ExactNumberInfo::PrecisionAndScale(78, 5)),
            collation: None,
            options: vec![],
        };

        let expected = ColumnDef {
            name: Ident::new("decimal_col"),
            data_type: sqlparser::ast::DataType::Decimal(ExactNumberInfo::PrecisionAndScale(75, 5)),
            collation: None,
            options: vec![],
        };

        assert_eq!(&column_def_clamp_precision(nullable_column), &expected);
        assert_eq!(&column_def_clamp_precision(expected.clone()), &expected);
    }
}

#[cfg(all(test, feature = "std"))]
mod std_tests {
    use arrow::datatypes::i256;

    use super::*;

    #[test]
    fn we_can_clamp_decimal_columns() {
        let column: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([0, 100, -10000].map(i256::from))
                .with_precision_and_scale(76, 5)
                .unwrap(),
        );

        let result = column_clamp_precision(column);
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([0, 100, -10000].map(i256::from))
                .with_precision_and_scale(75, 5)
                .unwrap(),
        );

        assert_eq!(&result, &expected);
    }

    #[test]
    fn we_can_truncate_decimal_columns() {
        let positive: String = std::iter::repeat("123456790")
            .flat_map(str::chars)
            .take(76)
            .collect();
        let mut negative = positive.clone();
        negative.insert(0, '-');
        let max = "57896044618658097711785492504343953926634992332820282019728792003956564819967";
        let min = "-57896044618658097711785492504343953926634992332820282019728792003956564819968";
        let column: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([
                i256::from(0),
                negative.parse().unwrap(),
                positive.parse().unwrap(),
                max.parse().unwrap(),
                min.parse().unwrap(),
            ])
            .with_precision_and_scale(76, 0)
            .unwrap(),
        );

        let result = column_clamp_precision(column);
        let expected_positive: String = std::iter::repeat("123456790")
            .flat_map(str::chars)
            .skip(1)
            .take(75)
            .collect();
        let mut expected_negative = expected_positive.clone();
        expected_negative.insert(0, '-');
        let expected_max =
            "896044618658097711785492504343953926634992332820282019728792003956564819967";
        let expected_min =
            "-896044618658097711785492504343953926634992332820282019728792003956564819968";
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([
                i256::from(0),
                expected_negative.parse().unwrap(),
                expected_positive.parse().unwrap(),
                expected_max.parse().unwrap(),
                expected_min.parse().unwrap(),
            ])
            .with_precision_and_scale(75, 0)
            .unwrap(),
        );

        assert_eq!(&result, &expected);
    }
}
