use std::sync::Arc;

use arrow::array::{
    ArrayRef,
    ArrowPrimitiveType,
    BooleanArray,
    Decimal128Array,
    Decimal256Array,
    Int16Array,
    Int32Array,
    Int64Array,
    Int8Array,
    PrimitiveArray,
    StringArray,
    TimestampMicrosecondArray,
    TimestampMillisecondArray,
    TimestampNanosecondArray,
    TimestampSecondArray,
};
use arrow::datatypes::{DataType, TimeUnit};
use sqlparser::ast::{ColumnDef, ColumnOption, ColumnOptionDef};

/// Returns the provided column def with the NOT NULL option.
pub fn column_def_not_null(mut column: ColumnDef) -> ColumnDef {
    if !column
        .options
        .iter()
        .any(|option| option.option == ColumnOption::NotNull)
    {
        column.options.push(ColumnOptionDef {
            name: None,
            option: ColumnOption::NotNull,
        });
    }

    column
}

/// Returns the provided primitive column with nulls replaced by defaults.
fn default_nulls_primitive<T: ArrowPrimitiveType>(array: &PrimitiveArray<T>) -> PrimitiveArray<T> {
    PrimitiveArray::from_iter_values(
        array
            .iter()
            .map(|value: Option<<T as ArrowPrimitiveType>::Native>| value.unwrap_or_default()),
    )
}

/// Returns the provided column with nulls replaced by defaults if null count > 0.
pub fn column_default_nulls(column: ArrayRef) -> ArrayRef {
    if column.null_count() > 0 {
        let column_type = column.data_type();
        let column: ArrayRef = match column_type {
            DataType::Int8 => Arc::new(default_nulls_primitive(
                column.as_any().downcast_ref::<Int8Array>().unwrap(),
            )),
            DataType::Int16 => Arc::new(default_nulls_primitive(
                column.as_any().downcast_ref::<Int16Array>().unwrap(),
            )),
            DataType::Int32 => Arc::new(default_nulls_primitive(
                column.as_any().downcast_ref::<Int32Array>().unwrap(),
            )),
            DataType::Int64 => Arc::new(default_nulls_primitive(
                column.as_any().downcast_ref::<Int64Array>().unwrap(),
            )),

            DataType::Decimal128(precision, scale) => Arc::new(
                default_nulls_primitive(column.as_any().downcast_ref::<Decimal128Array>().unwrap())
                    .with_precision_and_scale(*precision, *scale)
                    .unwrap(),
            ),
            DataType::Decimal256(precision, scale) => Arc::new(
                default_nulls_primitive(column.as_any().downcast_ref::<Decimal256Array>().unwrap())
                    .with_precision_and_scale(*precision, *scale)
                    .unwrap(),
            ),
            DataType::Timestamp(TimeUnit::Second, timezone) => Arc::new(
                default_nulls_primitive(
                    column
                        .as_any()
                        .downcast_ref::<TimestampSecondArray>()
                        .unwrap(),
                )
                .with_timezone_opt(timezone.clone()),
            ),
            DataType::Timestamp(TimeUnit::Millisecond, timezone) => Arc::new(
                default_nulls_primitive(
                    column
                        .as_any()
                        .downcast_ref::<TimestampMillisecondArray>()
                        .unwrap(),
                )
                .with_timezone_opt(timezone.clone()),
            ),
            DataType::Timestamp(TimeUnit::Microsecond, timezone) => Arc::new(
                default_nulls_primitive(
                    column
                        .as_any()
                        .downcast_ref::<TimestampMicrosecondArray>()
                        .unwrap(),
                )
                .with_timezone_opt(timezone.clone()),
            ),
            DataType::Timestamp(TimeUnit::Nanosecond, timezone) => Arc::new(
                default_nulls_primitive(
                    column
                        .as_any()
                        .downcast_ref::<TimestampNanosecondArray>()
                        .unwrap(),
                )
                .with_timezone_opt(timezone.clone()),
            ),
            DataType::Boolean => Arc::new(
                column
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap()
                    .iter()
                    .map(|element| Some(element.unwrap_or(false)))
                    .collect::<BooleanArray>(),
            ),
            DataType::Utf8 => Arc::new(StringArray::from_iter_values(
                column
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .iter()
                    .map(|element| element.unwrap_or("")),
            )),
            _ => unimplemented!(),
        };

        column
    } else {
        column
    }
}

#[cfg(test)]
mod tests {
    use arrow::buffer::NullBuffer;
    use arrow::datatypes::{
        ArrowNativeType,
        ArrowNativeTypeOp,
        Decimal128Type,
        Decimal256Type,
        DecimalType,
        Int16Type,
        Int32Type,
        Int64Type,
        Int8Type,
        TimestampMicrosecondType,
        TimestampMillisecondType,
        TimestampNanosecondType,
        TimestampSecondType,
    };
    use sqlparser::ast::Ident;

    use super::*;

    #[test]
    fn we_can_make_column_def_not_null() {
        let nullable_column = ColumnDef {
            name: Ident::new("bigint_col"),
            data_type: sqlparser::ast::DataType::BigInt(None),
            collation: None,
            options: vec![ColumnOptionDef {
                name: None,
                option: ColumnOption::Comment("hello, world!".to_string()),
            }],
        };

        let expected = ColumnDef {
            name: Ident::new("bigint_col"),
            data_type: sqlparser::ast::DataType::BigInt(None),
            collation: None,
            options: vec![
                ColumnOptionDef {
                    name: None,
                    option: ColumnOption::Comment("hello, world!".to_string()),
                },
                ColumnOptionDef {
                    name: None,
                    option: ColumnOption::NotNull,
                },
            ],
        };

        assert_eq!(&column_def_not_null(nullable_column), &expected);
        assert_eq!(&column_def_not_null(expected.clone()), &expected);
    }

    fn we_can_default_nulls_in_primitive_column<T: ArrowPrimitiveType>() {
        let column: ArrayRef = Arc::new(PrimitiveArray::<T>::from_iter_values_with_nulls(
            (0..5usize).map(|value| T::Native::from_usize(value).unwrap()),
            Some(NullBuffer::from(vec![true, true, false, false, true])),
        ));
        assert!(column.is_nullable());
        assert_eq!(column.null_count(), 2);

        let result = column_default_nulls(column);
        let expected = Arc::new(PrimitiveArray::<T>::from_iter_values([
            T::Native::ZERO,
            T::Native::ONE,
            T::Native::ZERO,
            T::Native::ZERO,
            T::Native::from_usize(4).unwrap(),
        ]));

        assert_eq!(result.as_ref(), expected.as_ref());
        assert!(!result.is_nullable());
        assert_eq!(result.null_count(), 0);
    }

    #[test]
    fn we_can_default_nulls_in_primitive_columns() {
        we_can_default_nulls_in_primitive_column::<Int8Type>();
        we_can_default_nulls_in_primitive_column::<Int16Type>();
        we_can_default_nulls_in_primitive_column::<Int32Type>();
        we_can_default_nulls_in_primitive_column::<Int64Type>();
        we_can_default_nulls_in_primitive_column::<TimestampSecondType>();
        we_can_default_nulls_in_primitive_column::<TimestampMillisecondType>();
        we_can_default_nulls_in_primitive_column::<TimestampMicrosecondType>();
        we_can_default_nulls_in_primitive_column::<TimestampNanosecondType>();
    }

    fn we_can_default_nulls_in_decimal_column<T: ArrowPrimitiveType + DecimalType>(
        precision: u8,
        scale: i8,
    ) {
        let column: ArrayRef = Arc::new(
            PrimitiveArray::<T>::from_iter_values_with_nulls(
                (0..5usize).map(|value| T::Native::from_usize(value).unwrap()),
                Some(NullBuffer::from(vec![true, true, false, false, true])),
            )
            .with_precision_and_scale(precision, scale)
            .unwrap(),
        );
        assert!(column.is_nullable());
        assert_eq!(column.null_count(), 2);

        let result = column_default_nulls(column);
        let expected = Arc::new(
            PrimitiveArray::<T>::from_iter_values([
                T::Native::ZERO,
                T::Native::ONE,
                T::Native::ZERO,
                T::Native::ZERO,
                T::Native::from_usize(4).unwrap(),
            ])
            .with_precision_and_scale(precision, scale)
            .unwrap(),
        );

        assert_eq!(result.as_ref(), expected.as_ref());
        assert!(!result.is_nullable());
        assert_eq!(result.null_count(), 0);
    }

    #[test]
    fn we_can_default_nulls_in_decimal_columns() {
        we_can_default_nulls_in_decimal_column::<Decimal128Type>(38, 0);
        we_can_default_nulls_in_decimal_column::<Decimal256Type>(76, 0);
        we_can_default_nulls_in_decimal_column::<Decimal128Type>(10, 5);
        we_can_default_nulls_in_decimal_column::<Decimal256Type>(20, 10);
    }

    #[test]
    fn we_can_default_nulls_in_string_column() {
        let column: ArrayRef = Arc::new(StringArray::from_iter([
            Some("lorem"),
            Some("ipsum"),
            None,
            None,
            Some(""),
        ]));
        assert!(column.is_nullable());
        assert_eq!(column.null_count(), 2);

        let result = column_default_nulls(column);
        let expected = Arc::new(StringArray::from_iter_values([
            "lorem", "ipsum", "", "", "",
        ]));

        assert_eq!(result.as_ref(), expected.as_ref());
        assert!(!result.is_nullable());
        assert_eq!(result.null_count(), 0);
    }

    #[test]
    fn we_can_default_nulls_in_boolean_column() {
        let column: ArrayRef = Arc::new(BooleanArray::from_iter([
            Some(true),
            Some(false),
            None,
            None,
            Some(true),
        ]));
        assert!(column.is_nullable());
        assert_eq!(column.null_count(), 2);

        let result = column_default_nulls(column);
        let expected = Arc::new(BooleanArray::from(vec![true, false, false, false, true]));

        assert_eq!(result.as_ref(), expected.as_ref());
        assert!(!result.is_nullable());
        assert_eq!(result.null_count(), 0);
    }
}
