use proof_of_sql::base::database::ColumnType;
use proof_of_sql::base::math::decimal::Precision;
use proof_of_sql::base::posql_time::{PoSQLTimeUnit, PoSQLTimeZone};
use snafu::Snafu;
use sqlparser::ast::{DataType, ExactNumberInfo, TimezoneInfo};

/// Error that occurs when encountering unsupported sqlparser `DataType`s.
#[derive(Debug, Snafu)]
pub enum UnsupportedColumnType {
    /// Decimal should have constrained precision and scale.
    #[snafu(display("decimal should have constrained precision and scale"))]
    UnconstrainedDecimal,
    /// Decimal precision should be between 1 and 75.
    #[snafu(display("decimal precision should be between 1 and 75, received: {precision}"))]
    DecimalPrecision {
        /// The invalid precision value.
        precision: u64,
    },
    /// Decimal scale should be between 0 and 127.
    #[snafu(display("decimal scale should between 0 and 127, received: {scale}"))]
    DecimalScale {
        /// The invalid scale value.
        scale: u64,
    },
    /// Data type parameter not supported
    #[snafu(display("data type supported, but not parameter: {data_type}"))]
    DataTypeParameter {
        /// The unsupported data type.
        data_type: DataType,
    },
    /// Data type not supported.
    #[snafu(display("data type not supported: {data_type}"))]
    DataType {
        /// The unsupported data type.
        data_type: DataType,
    },
}

/// Convert sqlparser decimal number info to proof-of-sql precision and scale.
fn sqlparser_number_info_to_proof_of_sql_precision_and_scale(
    number_info: &ExactNumberInfo,
) -> Result<(Precision, i8), UnsupportedColumnType> {
    let (precision, scale) = match number_info {
        // Postgres defines Numerics with no precision and scale as "unconstrained".
        ExactNumberInfo::None => Err(UnsupportedColumnType::UnconstrainedDecimal),
        // Postgres defines Numerics with no scale as 0-scale.
        ExactNumberInfo::Precision(p) => Ok((*p, 0)),
        ExactNumberInfo::PrecisionAndScale(p, s) => Ok((*p, *s)),
    }?;

    let precision = u8::try_from(precision)
        .map_err(|_| UnsupportedColumnType::DecimalPrecision { precision })
        .and_then(|p| {
            Precision::new(p).map_err(|_| UnsupportedColumnType::DecimalPrecision { precision })
        })?;

    let scale = i8::try_from(scale).map_err(|_| UnsupportedColumnType::DecimalScale { scale })?;

    Ok((precision, scale))
}

/// Convert sqlparser data type to proof-of-sql column type.
pub fn sqlparser_data_type_to_proof_of_sql_column_type(
    sqlparser_type: &DataType,
) -> Result<ColumnType, UnsupportedColumnType> {
    match sqlparser_type {
        DataType::Boolean => Ok(ColumnType::Boolean),
        DataType::TinyInt(None) => Ok(ColumnType::TinyInt),
        DataType::SmallInt(None) => Ok(ColumnType::SmallInt),
        DataType::Int(None) | DataType::Integer(None) => Ok(ColumnType::Int),
        DataType::BigInt(None) => Ok(ColumnType::BigInt),
        DataType::Varchar(None) => Ok(ColumnType::VarChar),
        DataType::Decimal(number_info) => {
            let (precision, scale) =
                sqlparser_number_info_to_proof_of_sql_precision_and_scale(number_info)?;
            Ok(ColumnType::Decimal75(precision, scale))
        }
        DataType::Timestamp(None, TimezoneInfo::None) => Ok(ColumnType::TimestampTZ(
            PoSQLTimeUnit::Millisecond,
            PoSQLTimeZone::utc(),
        )),
        DataType::Binary(None) => Ok(ColumnType::VarBinary),
        DataType::TinyInt(_)
        | DataType::SmallInt(_)
        | DataType::Int(_)
        | DataType::Integer(_)
        | DataType::BigInt(_)
        | DataType::Varchar(_)
        | DataType::Binary(_)
        | DataType::Timestamp(..) => Err(UnsupportedColumnType::DataTypeParameter {
            data_type: sqlparser_type.clone(),
        }),
        DataType::Bool
        | DataType::Uuid
        | DataType::Float(_)
        | DataType::UnsignedTinyInt(_)
        | DataType::MediumInt(_)
        | DataType::Int2(_)
        | DataType::UnsignedInt2(_)
        | DataType::UnsignedSmallInt(_)
        | DataType::UnsignedMediumInt(_)
        | DataType::UnsignedInt(_)
        | DataType::Int4(_)
        | DataType::UnsignedInt4(_)
        | DataType::UnsignedInteger(_)
        | DataType::Int8(_)
        | DataType::Int64
        | DataType::UnsignedBigInt(_)
        | DataType::UnsignedInt8(_)
        | DataType::Float4
        | DataType::Float64
        | DataType::Real
        | DataType::Float8
        | DataType::Double
        | DataType::DoublePrecision
        | DataType::Character(_)
        | DataType::Char(_)
        | DataType::CharacterVarying(_)
        | DataType::CharVarying(_)
        | DataType::Nvarchar(_)
        | DataType::CharacterLargeObject(_)
        | DataType::CharLargeObject(_)
        | DataType::Clob(_)
        | DataType::String(_)
        | DataType::Text
        | DataType::Numeric(_)
        | DataType::BigNumeric(_)
        | DataType::BigDecimal(_)
        | DataType::Dec(_)
        | DataType::Date
        | DataType::Time(..)
        | DataType::Datetime(_)
        | DataType::Varbinary(_)
        | DataType::Blob(_)
        | DataType::Bytes(_)
        | DataType::Interval
        | DataType::JSON
        | DataType::JSONB
        | DataType::Regclass
        | DataType::Bytea
        | DataType::Custom(..)
        | DataType::Array(_)
        | DataType::Enum(_)
        | DataType::Set(_)
        | DataType::Struct(_)
        | DataType::Unspecified => Err(UnsupportedColumnType::DataType {
            data_type: sqlparser_type.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use sqlparser::ast::{CharacterLength, TimezoneInfo};

    use super::*;

    #[test]
    fn we_cannot_convert_sqlparser_bytea() {
        let t = DataType::Bytea;
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&t),
            Err(UnsupportedColumnType::DataType { .. })
        ));
    }

    #[test]
    fn we_can_convert_simple_types_to_proof_of_sql() {
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::Boolean).unwrap(),
            ColumnType::Boolean,
        );

        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::TinyInt(None)).unwrap(),
            ColumnType::TinyInt,
        );

        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::SmallInt(None)).unwrap(),
            ColumnType::SmallInt,
        );

        assert_eq!(
            [DataType::Int(None), DataType::Integer(None),]
                .iter()
                .map(sqlparser_data_type_to_proof_of_sql_column_type)
                .map(Result::unwrap)
                .all_equal_value()
                .unwrap(),
            ColumnType::Int
        );

        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::BigInt(None)).unwrap(),
            ColumnType::BigInt,
        );

        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::Varchar(None)).unwrap(),
            ColumnType::VarChar,
        );

        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::Binary(None)).unwrap(),
            ColumnType::VarBinary,
        );

        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::Timestamp(
                None,
                TimezoneInfo::None
            ))
            .unwrap(),
            ColumnType::TimestampTZ(PoSQLTimeUnit::Millisecond, PoSQLTimeZone::utc())
        );
    }

    #[test]
    fn we_cannot_convert_sqlparser_type_with_unsupported_parameter() {
        [
            DataType::TinyInt(Some(0)),
            DataType::SmallInt(Some(1)),
            DataType::Int(Some(2)),
            DataType::Integer(Some(3)),
            DataType::BigInt(Some(4)),
            DataType::Varchar(Some(CharacterLength::Max)),
            DataType::Binary(Some(6)),
            DataType::Timestamp(Some(7), TimezoneInfo::None),
            DataType::Timestamp(None, TimezoneInfo::Tz),
        ]
        .iter()
        .for_each(|data_type_with_unsupported_parameter| {
            assert!(matches!(
                sqlparser_data_type_to_proof_of_sql_column_type(
                    data_type_with_unsupported_parameter
                ),
                Err(UnsupportedColumnType::DataTypeParameter { .. })
            ));
        })
    }

    #[test]
    fn we_can_convert_sqlparser_decimals_to_proof_of_sql() {
        let full_decimal = DataType::Decimal(ExactNumberInfo::PrecisionAndScale(75, 10));
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&full_decimal).unwrap(),
            ColumnType::Decimal75(Precision::new(75).unwrap(), 10)
        );

        let decimal_with_precision = DataType::Decimal(ExactNumberInfo::Precision(38));
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&decimal_with_precision).unwrap(),
            ColumnType::Decimal75(Precision::new(38).unwrap(), 0)
        );
    }

    #[test]
    fn we_cannot_convert_sqlparser_decimals_without_precision() {
        let unconstrained_decimal = DataType::Decimal(ExactNumberInfo::None);
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&unconstrained_decimal),
            Err(UnsupportedColumnType::UnconstrainedDecimal),
        ));
    }

    #[test]
    fn we_cannot_convert_sqlparser_decimals_with_out_of_bounds_precision() {
        let full_decimal_outside_u8 = DataType::Decimal(ExactNumberInfo::PrecisionAndScale(257, 0));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&full_decimal_outside_u8),
            Err(UnsupportedColumnType::DecimalPrecision { .. }),
        ));

        let decimal_precision_outside_u8 = DataType::Decimal(ExactNumberInfo::Precision(1000));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&decimal_precision_outside_u8),
            Err(UnsupportedColumnType::DecimalPrecision { .. }),
        ));

        let full_decimal_above_75 = DataType::Decimal(ExactNumberInfo::PrecisionAndScale(76, 0));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&full_decimal_above_75),
            Err(UnsupportedColumnType::DecimalPrecision { .. }),
        ));

        let decimal_precision_above_75 = DataType::Decimal(ExactNumberInfo::Precision(100));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&decimal_precision_above_75),
            Err(UnsupportedColumnType::DecimalPrecision { .. }),
        ));
    }

    #[test]
    fn we_cannot_convert_sqlparser_decimals_with_out_of_bounds_scale() {
        let decimal_scale_outside_i8 =
            DataType::Decimal(ExactNumberInfo::PrecisionAndScale(75, 128));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&decimal_scale_outside_i8),
            Err(UnsupportedColumnType::DecimalScale { .. }),
        ));
    }

    #[test]
    fn we_cannot_convert_unsupported_sqlparser_types() {
        let unsupported_data_type = DataType::Float64;
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&unsupported_data_type),
            Err(UnsupportedColumnType::DataType { .. }),
        ));
    }
}
