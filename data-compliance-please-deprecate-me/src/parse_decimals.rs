use std::str::FromStr;
use std::sync::Arc;

use arrow::array::{ArrayRef, Decimal256Array, GenericStringArray, OffsetSizeTrait};
use arrow::datatypes::{i256, DataType as ArrowDataType};
use arrow::error::ArrowError;
use bigdecimal::{BigDecimal, Num, ParseBigDecimalError, Signed};
use num_bigint::BigInt;
use snafu::Snafu;
use sqlparser::ast::{DataType as SqlparserDataType, ExactNumberInfo};

use crate::decimal_precision::MAX_PRECISION;

/// Errors that can occur when parsing string columns to decimal columns.
#[derive(Debug, Snafu)]
pub enum ParseDecimalsError {
    /// Unable to parse string value to BigDecimal.
    #[snafu(display("unable to parse string value to BigDecimal: {error}"))]
    BigDecimal {
        /// The source bigdecimal error.
        error: ParseBigDecimalError,
    },

    /// Unable to cast string value to decimal256.
    #[snafu(display("unable to cast string value to Decimal256: {error}"))]
    Cast {
        /// The source decimal256 error.
        error: ArrowError,
    },
}

impl From<ParseBigDecimalError> for ParseDecimalsError {
    fn from(error: ParseBigDecimalError) -> Self {
        ParseDecimalsError::BigDecimal { error }
    }
}

impl From<ArrowError> for ParseDecimalsError {
    fn from(error: ArrowError) -> Self {
        ParseDecimalsError::Cast { error }
    }
}

/// Casting a [`StringArray`] or [`LargeStringArray`] to a vector of [`Option<i256>`]s.
fn string_array_to_i256<O: OffsetSizeTrait>(
    column: ArrayRef,
    scale: i8,
) -> Result<Vec<Option<i256>>, ParseBigDecimalError> {
    column
        .as_any()
        .downcast_ref::<GenericStringArray<O>>()
        .unwrap()
        .iter()
        .map(|maybe_string| {
            maybe_string
                .map(|string| -> Result<i256, ParseBigDecimalError> {
                    let (bigint, _) = BigDecimal::from_str(string)?
                        .normalized()
                        .with_scale(scale as i64)
                        .into_bigint_and_exponent();
                    let string_representation = bigint.to_str_radix(10);
                    let truncated_string_representation: String = if bigint.is_negative() {
                        let actual_precision = string_representation.len() - 1;
                        let num_skipped_chars =
                            (actual_precision as i8 - MAX_PRECISION as i8 + 1i8).max(1) as usize;
                        std::iter::once('-')
                            .chain(string_representation.chars().skip(num_skipped_chars))
                            .collect()
                    } else {
                        let actual_precision = string_representation.len();
                        let num_skipped_chars =
                            (actual_precision as i8 - MAX_PRECISION as i8).max(0) as usize;
                        string_representation
                            .chars()
                            .skip(num_skipped_chars)
                            .collect()
                    };
                    Ok(i256::from_string(&truncated_string_representation).expect(
                        "previous string representation minus one digit should still be valid",
                    ))
                })
                .transpose()
        })
        .collect::<Result<Vec<_>, ParseBigDecimalError>>()
}

fn truncate_string_array<O: OffsetSizeTrait>(
    column: ArrayRef,
    scale: i8,
) -> Result<ArrayRef, ParseBigDecimalError> {
    let truncated_strings = column
        .as_any()
        .downcast_ref::<GenericStringArray<O>>()
        .unwrap()
        .iter()
        .map(|maybe_string| {
            maybe_string
                .map(|string| -> Result<String, ParseBigDecimalError> {
                    let (bigint, _) = BigDecimal::from_str(string)?
                        .normalized()
                        .with_scale(scale as i64)
                        .into_bigint_and_exponent();
                    let string_representation = bigint.to_str_radix(10);
                    let truncated_string_representation: String = if bigint.is_negative() {
                        let actual_precision = string_representation.len() - 1;
                        let num_skipped_chars =
                            (actual_precision as i8 - MAX_PRECISION as i8 + 1i8).max(1) as usize;
                        std::iter::once('-')
                            .chain(string_representation.chars().skip(num_skipped_chars))
                            .collect()
                    } else {
                        let actual_precision = string_representation.len();
                        let num_skipped_chars =
                            (actual_precision as i8 - MAX_PRECISION as i8).max(0) as usize;
                        string_representation
                            .chars()
                            .skip(num_skipped_chars)
                            .collect()
                    };
                    // Now cast the truncated string back to a BigDecimal to ensure it is valid
                    let truncated_bigint =
                        BigInt::from_str_radix(&truncated_string_representation, 10)
                            .expect("truncated string representation should still be valid");
                    let truncated_bigdecimal = BigDecimal::new(truncated_bigint, scale.into());
                    Ok(truncated_bigdecimal.to_string())
                })
                .transpose()
        })
        .collect::<Result<Vec<_>, ParseBigDecimalError>>()?;
    Ok(Arc::new(GenericStringArray::<O>::from(truncated_strings)))
}

/// Returns the provided column with strings parsed to decimals if the column type is string and
/// the target type is decimal.
///
/// Errors if the cast fails.
pub fn column_parse_decimals_fallible(
    column: ArrayRef,
    target_type: &SqlparserDataType,
) -> Result<ArrayRef, ParseDecimalsError> {
    match (column.data_type(), target_type) {
        (
            ArrowDataType::Utf8 | ArrowDataType::LargeUtf8,
            SqlparserDataType::Numeric(number_info)
            | SqlparserDataType::Decimal(number_info)
            | SqlparserDataType::BigNumeric(number_info)
            | SqlparserDataType::BigDecimal(number_info)
            | SqlparserDataType::Dec(number_info),
        ) => {
            let (precision, scale) = match number_info {
                ExactNumberInfo::None => (MAX_PRECISION, 0),
                ExactNumberInfo::Precision(p) => ((*p as u8).min(MAX_PRECISION), 0),
                ExactNumberInfo::PrecisionAndScale(p, s) => {
                    ((*p as u8).min(MAX_PRECISION), *s as i8)
                }
            };

            match column.data_type() {
                ArrowDataType::Utf8 => {
                    let column: ArrayRef = Arc::new(
                        Decimal256Array::from_iter(string_array_to_i256::<i32>(column, scale)?)
                            .with_precision_and_scale(precision, scale)?,
                    );
                    Ok(column)
                }
                ArrowDataType::LargeUtf8 => {
                    let column: ArrayRef = Arc::new(
                        Decimal256Array::from_iter(string_array_to_i256::<i64>(column, scale)?)
                            .with_precision_and_scale(precision, scale)?,
                    );
                    Ok(column)
                }
                _ => unreachable!(),
            }
        }
        _ => Ok(column),
    }
}

/// Returns the provided column with strings parsed to decimals if the column type is string and
/// the target type is decimal.
///
/// Panics if the cast fails.
pub fn column_parse_decimals_unchecked(
    column: ArrayRef,
    target_type: &SqlparserDataType,
) -> ArrayRef {
    column_parse_decimals_fallible(column, target_type)
        .expect("string column unable to parse to decimals")
}

/// Parse bigdecimal columns from a string column, truncating the most significant digits if the
/// precision is too high, and returning an error if the string cannot be parsed. After that cast
/// the bigdecimals back to strings.
pub fn column_truncate_decimals_fallible(
    column: ArrayRef,
    target_type: &SqlparserDataType,
) -> Result<ArrayRef, ParseDecimalsError> {
    match (column.data_type(), target_type) {
        (
            ArrowDataType::Utf8 | ArrowDataType::LargeUtf8,
            SqlparserDataType::Numeric(number_info)
            | SqlparserDataType::Decimal(number_info)
            | SqlparserDataType::BigNumeric(number_info)
            | SqlparserDataType::BigDecimal(number_info)
            | SqlparserDataType::Dec(number_info),
        ) => {
            let (_precision, scale) = match number_info {
                ExactNumberInfo::None => (MAX_PRECISION, 0),
                ExactNumberInfo::Precision(p) => ((*p as u8).min(MAX_PRECISION), 0),
                ExactNumberInfo::PrecisionAndScale(p, s) => {
                    ((*p as u8).min(MAX_PRECISION), *s as i8)
                }
            };

            match column.data_type() {
                ArrowDataType::Utf8 => Ok(truncate_string_array::<i32>(column, scale)?),
                ArrowDataType::LargeUtf8 => Ok(truncate_string_array::<i64>(column, scale)?),
                _ => unreachable!(),
            }
        }
        _ => Ok(column),
    }
}

/// Parse bigdecimal columns from a string column, truncating the most significant digits if the
/// precision is too high, and returning an error if the string cannot be parsed. After that cast
/// the bigdecimals back to strings.
pub fn column_truncate_decimals_unchecked(
    column: ArrayRef,
    target_type: &SqlparserDataType,
) -> ArrayRef {
    column_truncate_decimals_fallible(column, target_type)
        .expect("string column unable to parse to decimals")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{Decimal256Array, LargeStringArray, StringArray};
    use arrow::datatypes::i256;

    use super::*;

    #[test]
    fn we_can_parse_decimals_without_truncation() {
        let max_number = "9".repeat(75);
        let mut min_number = max_number.clone();
        min_number.insert(0, '-');
        let column: ArrayRef = Arc::new(LargeStringArray::from_iter_values([
            "0",
            &max_number,
            &min_number,
        ]));

        let data_type = SqlparserDataType::Numeric(ExactNumberInfo::PrecisionAndScale(75, 0));

        let expected: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([
                i256::from_i128(0),
                i256::from_str(&max_number).unwrap(),
                i256::from_str(&min_number).unwrap(),
            ])
            .with_precision_and_scale(75, 0)
            .unwrap(),
        );

        assert_eq!(
            &column_parse_decimals_unchecked(column, &data_type),
            &expected
        );

        let column: ArrayRef = Arc::new(LargeStringArray::from_iter_values(["0", "-10.5", "2e4"]));
        let data_type = SqlparserDataType::Decimal(ExactNumberInfo::PrecisionAndScale(10, 2));
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([
                i256::from_i128(0),
                i256::from_i128(-1050),
                i256::from_i128(2000000),
            ])
            .with_precision_and_scale(10, 2)
            .unwrap(),
        );

        assert_eq!(
            &column_parse_decimals_unchecked(column, &data_type),
            &expected
        );

        // Trailing zeros && rounding
        let column: ArrayRef = Arc::new(StringArray::from_iter_values([
            "0.000",
            "-10.5000000000",
            "2.00000e-4",
        ]));
        let data_type = SqlparserDataType::Decimal(ExactNumberInfo::PrecisionAndScale(10, 3));
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([
                i256::from_i128(0),
                i256::from_i128(-10500),
                i256::from_i128(0),
            ])
            .with_precision_and_scale(10, 3)
            .unwrap(),
        );
        assert_eq!(
            &column_parse_decimals_unchecked(column, &data_type),
            &expected
        );
    }

    #[test]
    fn we_cannot_parse_nondecimals() {
        let column: ArrayRef = Arc::new(LargeStringArray::from_iter_values([
            "0",
            "not a decimal",
            "200",
        ]));

        let data_type = SqlparserDataType::Decimal(ExactNumberInfo::PrecisionAndScale(75, 0));
        assert!(matches!(
            column_parse_decimals_fallible(column, &data_type),
            Err(ParseDecimalsError::BigDecimal { .. })
        ))
    }

    #[test]
    fn we_can_parse_out_of_bounds_decimals_and_truncate_them() {
        let excessive_precision = "1234567890".repeat(8);
        let negative_excessive_precision = format!("-{}", &excessive_precision);
        let column: ArrayRef = Arc::new(StringArray::from_iter_values([
            &excessive_precision,
            &negative_excessive_precision,
        ]));

        let data_type = SqlparserDataType::Numeric(ExactNumberInfo::PrecisionAndScale(75, 0));
        // Truncate the most significant digits
        let expected_string = format!("{}{}", "67890", "1234567890".repeat(7));
        let negative_expected_string = format!("-{}", &expected_string);
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([
                i256::from_str(&expected_string).unwrap(),
                i256::from_str(&negative_expected_string).unwrap(),
            ])
            .with_precision_and_scale(75, 0)
            .unwrap(),
        );

        assert_eq!(
            &column_parse_decimals_unchecked(column, &data_type),
            &expected
        );

        // Scientific notation
        let column: ArrayRef = Arc::new(LargeStringArray::from_iter_values(["1e100", "-1e100"]));
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from_iter_values([i256::from_i128(0), i256::from_i128(0)])
                .with_precision_and_scale(75, 0)
                .unwrap(),
        );

        assert_eq!(
            &column_parse_decimals_unchecked(column, &data_type),
            &expected
        );
    }

    #[test]
    fn we_cannot_parse_and_truncate_nondecimals_and_convert_them_back_to_strings() {
        let column: ArrayRef = Arc::new(LargeStringArray::from_iter_values([
            "0",
            "not a decimal",
            "200",
        ]));

        let data_type = SqlparserDataType::Decimal(ExactNumberInfo::PrecisionAndScale(75, 0));
        assert!(matches!(
            column_truncate_decimals_fallible(column, &data_type),
            Err(ParseDecimalsError::BigDecimal { .. })
        ))
    }

    #[test]
    fn we_can_parse_decimals_and_convert_them_back_to_strings() {
        let column: ArrayRef = Arc::new(LargeStringArray::from_iter_values([
            "0.000",
            "-10.5000000000",
            "2.00000e-4",
        ]));
        let data_type = SqlparserDataType::Decimal(ExactNumberInfo::PrecisionAndScale(10, 3));
        let expected: ArrayRef = Arc::new(LargeStringArray::from_iter_values([
            "0.000", "-10.500", "0.000",
        ]));
        assert_eq!(
            &column_truncate_decimals_unchecked(column, &data_type),
            &expected
        );
    }

    #[test]
    fn we_can_parse_out_of_bounds_decimals_truncate_them_and_convert_them_back_to_strings() {
        let excessive_precision = "1234567890".repeat(8);
        let negative_excessive_precision = format!("-{}", &excessive_precision);
        let column: ArrayRef = Arc::new(StringArray::from_iter_values([
            &excessive_precision,
            &negative_excessive_precision,
        ]));

        let data_type = SqlparserDataType::Numeric(ExactNumberInfo::PrecisionAndScale(75, 0));
        // Truncate the most significant digits
        let expected_string = format!("{}{}", "67890", "1234567890".repeat(7));
        let negative_expected_string = format!("-{}", &expected_string);
        let expected: ArrayRef = Arc::new(StringArray::from_iter_values([
            &expected_string,
            &negative_expected_string,
        ]));

        assert_eq!(
            &column_truncate_decimals_unchecked(column, &data_type),
            &expected
        );

        // Scientific notation
        let column: ArrayRef = Arc::new(LargeStringArray::from_iter_values(["1e100", "-1e100"]));
        let expected: ArrayRef = Arc::new(LargeStringArray::from_iter_values(["0", "0"]));

        assert_eq!(
            &column_truncate_decimals_unchecked(column, &data_type),
            &expected
        );
    }
}
