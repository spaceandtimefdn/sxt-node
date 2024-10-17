use alloc::string::ToString;
use alloc::sync::Arc;

use arrow::array::{
    ArrayRef,
    BooleanArray,
    Decimal128Array,
    Decimal256Array,
    Int16Array,
    Int32Array,
    Int64Array,
    Int8Array,
    StringArray,
    TimestampMicrosecondArray,
    TimestampMillisecondArray,
    TimestampNanosecondArray,
    TimestampSecondArray,
};
use arrow::datatypes::{DataType, TimeUnit};
use proof_of_sql::base::math::decimal::Precision;
use proof_of_sql_parser::posql_time::{PoSQLTimeUnit, PoSQLTimeZone, PoSQLTimestampError};
use snafu::Snafu;

use crate::i256_conversion::{arrow_i256_to_u256, u256_to_arrow_i256};
use crate::OnChainColumn;

/// Errors that can occur when converting from an arrow `ArrayRef` to [`OnChainColumn`].
#[derive(Debug, Snafu)]
pub enum ArrowToOnChainColumnError {
    /// Arrow type is not supported by sxt-node.
    #[snafu(display("arrow type {data_type} is not supported by sxt-node"))]
    UnsupportedType {
        /// Unsupported data type that was encountered.
        data_type: DataType,
    },
    /// Arrow type is not supported by sxt-node.
    #[snafu(display("nullable columns are not supported by sxt-node"))]
    UnsupportedNull,
    /// Arrow type failed to convert to proof-of-sql timestamp.
    #[snafu(display("arrow type failed to convert to proof-of-sql timestamp type: {error}"))]
    InvalidTimestamp {
        /// Error encountered by timestamp conversion.
        error: PoSQLTimestampError,
    },
}

impl From<PoSQLTimestampError> for ArrowToOnChainColumnError {
    fn from(error: PoSQLTimestampError) -> Self {
        ArrowToOnChainColumnError::InvalidTimestamp { error }
    }
}

impl TryFrom<&ArrayRef> for OnChainColumn {
    type Error = ArrowToOnChainColumnError;
    fn try_from(value: &ArrayRef) -> Result<Self, Self::Error> {
        if value.is_nullable() {
            return Err(ArrowToOnChainColumnError::UnsupportedNull);
        }

        match &value.data_type() {
            // Arrow uses a bit-packed representation for booleans.
            // Hence we need to unpack the bits to get the actual boolean values.
            DataType::Boolean => Ok(Self::Boolean(
                value
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap()
                    .iter()
                    .collect::<Option<Vec<bool>>>()
                    .ok_or(ArrowToOnChainColumnError::UnsupportedNull)?,
            )),
            DataType::Int8 => Ok(Self::TinyInt(
                value
                    .as_any()
                    .downcast_ref::<Int8Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            )),
            DataType::Int16 => Ok(Self::SmallInt(
                value
                    .as_any()
                    .downcast_ref::<Int16Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            )),
            DataType::Int32 => Ok(Self::Int(
                value
                    .as_any()
                    .downcast_ref::<Int32Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            )),
            DataType::Int64 => Ok(Self::BigInt(
                value
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            )),
            DataType::Decimal128(38, 0) => Ok(Self::Int128(
                value
                    .as_any()
                    .downcast_ref::<Decimal128Array>()
                    .unwrap()
                    .values()
                    .to_vec(),
            )),
            DataType::Decimal256(precision, scale) if *precision <= 75 => Ok(Self::Decimal75(
                Precision::new(*precision).expect("precision is less than 76"),
                *scale,
                value
                    .as_any()
                    .downcast_ref::<Decimal256Array>()
                    .unwrap()
                    .values()
                    .into_iter()
                    .copied()
                    .map(arrow_i256_to_u256)
                    .collect(),
            )),
            DataType::Utf8 => Ok(Self::VarChar(
                value
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .iter()
                    .map(|s| s.unwrap().to_string())
                    .collect(),
            )),
            DataType::Timestamp(time_unit, timezone) => {
                let (time_unit, timestamps) = match time_unit {
                    TimeUnit::Second => {
                        let array = value
                            .as_any()
                            .downcast_ref::<TimestampSecondArray>()
                            .expect(
                            "This cannot fail, all Arrow TimeUnits are mapped to PoSQL TimeUnits",
                        );
                        let timestamps = array.values().iter().copied().collect::<Vec<i64>>();
                        (PoSQLTimeUnit::Second, timestamps)
                    }
                    TimeUnit::Millisecond => {
                        let array = value
                            .as_any()
                            .downcast_ref::<TimestampMillisecondArray>()
                            .expect(
                            "This cannot fail, all Arrow TimeUnits are mapped to PoSQL TimeUnits",
                        );
                        let timestamps = array.values().iter().copied().collect::<Vec<i64>>();
                        (PoSQLTimeUnit::Millisecond, timestamps)
                    }
                    TimeUnit::Microsecond => {
                        let array = value
                            .as_any()
                            .downcast_ref::<TimestampMicrosecondArray>()
                            .expect(
                            "This cannot fail, all Arrow TimeUnits are mapped to PoSQL TimeUnits",
                        );
                        let timestamps = array.values().iter().copied().collect::<Vec<i64>>();
                        (PoSQLTimeUnit::Microsecond, timestamps)
                    }
                    TimeUnit::Nanosecond => {
                        let array = value
                            .as_any()
                            .downcast_ref::<TimestampNanosecondArray>()
                            .expect(
                            "This cannot fail, all Arrow TimeUnits are mapped to PoSQL TimeUnits",
                        );
                        let timestamps = array.values().iter().copied().collect::<Vec<i64>>();
                        (PoSQLTimeUnit::Nanosecond, timestamps)
                    }
                };

                Ok(Self::TimestampTZ(
                    time_unit,
                    PoSQLTimeZone::try_from(timezone)?,
                    timestamps,
                ))
            }
            &data_type => Err(ArrowToOnChainColumnError::UnsupportedType {
                data_type: data_type.clone(),
            }),
        }
    }
}

impl From<OnChainColumn> for ArrayRef {
    fn from(value: OnChainColumn) -> Self {
        match value {
            OnChainColumn::Boolean(col) => Arc::new(BooleanArray::from(col)),
            OnChainColumn::TinyInt(col) => Arc::new(Int8Array::from(col)),
            OnChainColumn::SmallInt(col) => Arc::new(Int16Array::from(col)),
            OnChainColumn::Int(col) => Arc::new(Int32Array::from(col)),
            OnChainColumn::BigInt(col) => Arc::new(Int64Array::from(col)),
            OnChainColumn::Int128(col) => Arc::new(
                Decimal128Array::from(col)
                    .with_precision_and_scale(38, 0)
                    .unwrap(),
            ),
            OnChainColumn::Decimal75(precision, scale, col) => {
                let converted_col = col.into_iter().map(u256_to_arrow_i256).collect::<Vec<_>>();

                Arc::new(
                    Decimal256Array::from(converted_col)
                        .with_precision_and_scale(precision.value(), scale)
                        .unwrap(),
                )
            }
            OnChainColumn::VarChar(col) => Arc::new(StringArray::from(col)),
            OnChainColumn::TimestampTZ(time_unit, timezone, col) => match time_unit {
                PoSQLTimeUnit::Second => {
                    Arc::new(TimestampSecondArray::from(col).with_timezone(timezone.to_string()))
                }
                PoSQLTimeUnit::Millisecond => Arc::new(
                    TimestampMillisecondArray::from(col).with_timezone(timezone.to_string()),
                ),
                PoSQLTimeUnit::Microsecond => Arc::new(
                    TimestampMicrosecondArray::from(col).with_timezone(timezone.to_string()),
                ),
                PoSQLTimeUnit::Nanosecond => Arc::new(
                    TimestampNanosecondArray::from(col).with_timezone(timezone.to_string()),
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use arrow::array::Float32Array;
    use arrow::datatypes::i256;
    use primitive_types::U256;

    use super::*;

    #[test]
    fn we_can_convert_simple_arrow_arrays_to_on_chain_column() {
        let data = vec![false, true, false, false];
        let array: ArrayRef = Arc::new(BooleanArray::from(data.clone()));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::Boolean(data)
        );
        let data = vec![1, 2, 3];
        let array: ArrayRef = Arc::new(Int8Array::from(data.clone()));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::TinyInt(data)
        );

        let data = vec![1, 10, -20, 30];
        let array: ArrayRef = Arc::new(Int16Array::from(data.clone()));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::SmallInt(data)
        );

        let data = vec![4, 5, 6];
        let array: ArrayRef = Arc::new(Int32Array::from(data.clone()));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::Int(data)
        );

        let data = vec![-7, -8, -9];
        let array: ArrayRef = Arc::new(Int64Array::from(data.clone()));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::BigInt(data)
        );

        let data = ["lorem", "ipsum", "dolor"].map(String::from).to_vec();
        let array: ArrayRef = Arc::new(StringArray::from(data.clone()));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::VarChar(data)
        );
    }

    #[test]
    fn we_can_convert_arrow_decimal_array_to_on_chain_column() {
        let data = [100, 1000, 100000i64];
        let array: ArrayRef = Arc::new(
            Decimal256Array::from(data.map(i256::from).to_vec())
                .with_precision_and_scale(75, 0)
                .unwrap(),
        );
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::Decimal75(
                Precision::new(75).unwrap(),
                0,
                data.map(U256::from).to_vec()
            )
        );

        let data = [100, 1000, 1234567890i64];
        let array: ArrayRef = Arc::new(
            Decimal256Array::from(data.map(i256::from).to_vec())
                .with_precision_and_scale(10, 10)
                .unwrap(),
        );
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::Decimal75(
                Precision::new(10).unwrap(),
                10,
                data.map(U256::from).to_vec()
            )
        );

        let data = [1, 0, 1i64];
        let array: ArrayRef = Arc::new(
            Decimal256Array::from(data.map(i256::from).to_vec())
                .with_precision_and_scale(1, -75)
                .unwrap(),
        );
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::Decimal75(
                Precision::new(1).unwrap(),
                -75,
                data.map(U256::from).to_vec()
            )
        );
    }

    #[test]
    fn we_can_convert_arrow_timestamp_array_to_on_chain_column() {
        let data = vec![1, 2, 3];
        let array: ArrayRef = Arc::new(TimestampSecondArray::from(data.clone()));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::TimestampTZ(PoSQLTimeUnit::Second, PoSQLTimeZone::Utc, data)
        );

        let data = vec![0, -1, -2];
        let array: ArrayRef =
            Arc::new(TimestampMillisecondArray::from(data.clone()).with_timezone("+00:00"));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::TimestampTZ(PoSQLTimeUnit::Millisecond, PoSQLTimeZone::Utc, data)
        );

        let data = vec![4, 5, 6];
        let array: ArrayRef =
            Arc::new(TimestampMicrosecondArray::from(data.clone()).with_timezone("+01:00"));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::TimestampTZ(
                PoSQLTimeUnit::Microsecond,
                PoSQLTimeZone::FixedOffset(3600),
                data
            )
        );

        let data = vec![-3, -4, -5];
        let array: ArrayRef =
            Arc::new(TimestampNanosecondArray::from(data.clone()).with_timezone("-01:00"));
        assert_eq!(
            OnChainColumn::try_from(&array).unwrap(),
            OnChainColumn::TimestampTZ(
                PoSQLTimeUnit::Nanosecond,
                PoSQLTimeZone::FixedOffset(-3600),
                data
            )
        );
    }

    #[test]
    fn we_cannot_convert_from_arrow_with_unsupported_type() {
        let array: ArrayRef = Arc::new(Float32Array::from(vec![1., 2., 3.]));
        assert!(matches!(
            OnChainColumn::try_from(&array),
            Err(ArrowToOnChainColumnError::UnsupportedType { .. })
        ));

        let array: ArrayRef = Arc::new(
            Decimal256Array::from([1, 2, 3].map(i256::from).to_vec())
                .with_precision_and_scale(76, 0)
                .unwrap(),
        );
        assert!(matches!(
            OnChainColumn::try_from(&array),
            Err(ArrowToOnChainColumnError::UnsupportedType { .. })
        ));
    }

    #[test]
    fn we_cannot_convert_from_nullable_arrow_array() {
        let array: ArrayRef = Arc::new(Int16Array::from(vec![Some(1), None]));
        assert!(matches!(
            OnChainColumn::try_from(&array),
            Err(ArrowToOnChainColumnError::UnsupportedNull { .. })
        ));
    }

    #[test]
    fn we_cannot_convert_from_arrow_timestamp_with_invalid_timezone() {
        let data = vec![-3, -4, -5];
        let array: ArrayRef =
            Arc::new(TimestampNanosecondArray::from(data.clone()).with_timezone("invalid"));
        assert!(matches!(
            OnChainColumn::try_from(&array),
            Err(ArrowToOnChainColumnError::InvalidTimestamp { .. })
        ));
    }

    #[test]
    fn we_can_convert_simple_on_chain_column_to_arrow() {
        let data = vec![false, true, false, false];
        let expected: ArrayRef = Arc::new(BooleanArray::from(data.clone()));
        // assert_eq! is not ArrayRef-friendly
        assert!(ArrayRef::from(OnChainColumn::Boolean(data)) == expected);

        let data = vec![1, 2, 3];
        let expected: ArrayRef = Arc::new(Int8Array::from(data.clone()));
        assert!(ArrayRef::from(OnChainColumn::TinyInt(data)) == expected,);

        let data = vec![1, 10, -20, 30];
        let expected: ArrayRef = Arc::new(Int16Array::from(data.clone()));
        assert!(ArrayRef::from(OnChainColumn::SmallInt(data)) == expected,);

        let data = vec![4, 5, 6];
        let expected: ArrayRef = Arc::new(Int32Array::from(data.clone()));
        assert!(ArrayRef::from(OnChainColumn::Int(data)) == expected,);

        let data = vec![-7, -8, -9];
        let expected: ArrayRef = Arc::new(Int64Array::from(data.clone()));
        assert!(ArrayRef::from(OnChainColumn::BigInt(data)) == expected,);

        let data = ["lorem", "ipsum", "dolor"].map(String::from).to_vec();
        let expected: ArrayRef = Arc::new(StringArray::from(data.clone()));
        assert!(ArrayRef::from(OnChainColumn::VarChar(data)) == expected,);
    }

    #[test]
    fn we_can_convert_on_chain_decimal_to_arrow() {
        let data = [100, 1000, 100000i64];
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from(data.map(i256::from).to_vec())
                .with_precision_and_scale(75, 0)
                .unwrap(),
        );
        assert!(
            ArrayRef::from(OnChainColumn::Decimal75(
                Precision::new(75).unwrap(),
                0,
                data.map(U256::from).to_vec()
            )) == expected,
        );

        let data = [100, 1000, 1234567890i64];
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from(data.map(i256::from).to_vec())
                .with_precision_and_scale(10, 10)
                .unwrap(),
        );
        assert!(
            ArrayRef::from(OnChainColumn::Decimal75(
                Precision::new(10).unwrap(),
                10,
                data.map(U256::from).to_vec()
            )) == expected,
        );

        let data = [1, 0, 1i64];
        let expected: ArrayRef = Arc::new(
            Decimal256Array::from(data.map(i256::from).to_vec())
                .with_precision_and_scale(1, -75)
                .unwrap(),
        );
        assert!(
            ArrayRef::from(OnChainColumn::Decimal75(
                Precision::new(1).unwrap(),
                -75,
                data.map(U256::from).to_vec()
            )) == expected
        );
    }

    #[test]
    fn we_can_convert_on_chain_timestamp_to_arrow() {
        let data = vec![1, 2, 3];
        let expected: ArrayRef =
            Arc::new(TimestampSecondArray::from(data.clone()).with_timezone("+00:00"));
        assert!(
            ArrayRef::from(OnChainColumn::TimestampTZ(
                PoSQLTimeUnit::Second,
                PoSQLTimeZone::Utc,
                data
            )) == expected
        );

        let data = vec![0, -1, -2];
        let expected: ArrayRef =
            Arc::new(TimestampMillisecondArray::from(data.clone()).with_timezone("+00:00"));
        assert!(
            ArrayRef::from(OnChainColumn::TimestampTZ(
                PoSQLTimeUnit::Millisecond,
                PoSQLTimeZone::Utc,
                data
            )) == expected
        );

        let data = vec![4, 5, 6];
        let expected: ArrayRef =
            Arc::new(TimestampMicrosecondArray::from(data.clone()).with_timezone("+01:00"));
        assert!(
            ArrayRef::from(OnChainColumn::TimestampTZ(
                PoSQLTimeUnit::Microsecond,
                PoSQLTimeZone::FixedOffset(3600),
                data
            )) == expected,
        );

        let data = vec![-3, -4, -5];
        let expected: ArrayRef =
            Arc::new(TimestampNanosecondArray::from(data.clone()).with_timezone("-01:00"));
        assert!(
            ArrayRef::from(OnChainColumn::TimestampTZ(
                PoSQLTimeUnit::Nanosecond,
                PoSQLTimeZone::FixedOffset(-3600),
                data
            )) == expected,
        );
    }
}
