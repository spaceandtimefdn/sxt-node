use proof_of_sql::base::{database::ColumnType, math::decimal::Precision};
use proof_of_sql_parser::posql_time::{PoSQLTimeUnit, PoSQLTimeZone};
use snafu::Snafu;
use sqlparser::ast::{DataType, ExactNumberInfo, TimezoneInfo};

/// Error that occurs when encountering unsupported sqlparser `DataType`s.
#[derive(Debug, Snafu)]
pub enum UnsupportedColumnType {
    /// Time precision should be 0, 3, or 6.
    #[snafu(display("time precision should be 0, 3, or 6, received: {precision}"))]
    TimestampPrecision {
        /// The invalid precision value.
        precision: u64,
    },
    /// Timestamp should be defined as timezone-aware.
    #[snafu(display("timestamp should be defined as timezone-aware"))]
    TimestampWithoutTimezone,
    /// Decimal/numeric should have constrained precision and scale.
    #[snafu(display("decimal/numeric should have constrained precision and scale"))]
    UnconstrainedDecimal,
    /// Decimal/numeric precision should be between 1 and 75.
    #[snafu(display(
        "decimal/numeric precision should be between 1 and 75, received: {precision}"
    ))]
    DecimalPrecision {
        /// The invalid precision value.
        precision: u64,
    },
    /// Decimal/numeric scale should be between 0 and 127.
    #[snafu(display("decimal/numeric scale should between 0 and 127, received: {scale}"))]
    DecimalScale {
        /// The invalid scale value.
        scale: u64,
    },
    /// Data type not supported.
    #[snafu(display("data type not supported: {data_type}"))]
    DataType {
        /// The unsupported data type.
        data_type: DataType,
    },
}

/// Convert sqlparser time type precision to proof-of-sql time unit.
fn sqlparser_precision_to_proof_of_sql_time_unit(
    precision: &Option<u64>,
) -> Result<PoSQLTimeUnit, UnsupportedColumnType> {
    match precision.as_ref() {
        Some(0) => Ok(PoSQLTimeUnit::Second),
        Some(3) => Ok(PoSQLTimeUnit::Millisecond),
        // Microseconds are the default resolution in postgres
        Some(6) | None => Ok(PoSQLTimeUnit::Microsecond),
        // Postgres does not support precision > 6
        Some(&precision) => Err(UnsupportedColumnType::TimestampPrecision { precision }),
    }
}

/// Convert sqlparser decimal/numeric number info to proof-of-sql precision and scale.
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
        DataType::Bool | DataType::Boolean => Ok(ColumnType::Boolean),
        DataType::TinyInt(_) => Ok(ColumnType::TinyInt),
        DataType::Int2(_) | DataType::SmallInt(_) => Ok(ColumnType::SmallInt),
        DataType::Int(_) | DataType::Int4(_) | DataType::Int64 | DataType::Integer(_) => {
            Ok(ColumnType::Int)
        }
        DataType::BigInt(_) | DataType::Int8(_) => Ok(ColumnType::BigInt),
        DataType::Character(_)
        | DataType::Char(_)
        | DataType::CharacterVarying(_)
        | DataType::CharVarying(_)
        | DataType::Varchar(_)
        | DataType::Nvarchar(_)
        | DataType::CharacterLargeObject(_)
        | DataType::CharLargeObject(_)
        | DataType::Clob(_)
        | DataType::String(_)
        | DataType::Text => Ok(ColumnType::VarChar),
        DataType::Numeric(number_info)
        | DataType::Decimal(number_info)
        | DataType::BigNumeric(number_info)
        | DataType::BigDecimal(number_info)
        | DataType::Dec(number_info) => {
            let (precision, scale) =
                sqlparser_number_info_to_proof_of_sql_precision_and_scale(number_info)?;

            Ok(ColumnType::Decimal75(precision, scale))
        }
        DataType::Datetime(precision) => sqlparser_precision_to_proof_of_sql_time_unit(precision)
            .map(|unit| ColumnType::TimestampTZ(unit, PoSQLTimeZone::Utc)),
        DataType::Timestamp(precision, timezone_info) => {
            if matches!(
                timezone_info,
                TimezoneInfo::None | TimezoneInfo::WithoutTimeZone
            ) {
                Err(UnsupportedColumnType::TimestampWithoutTimezone)?
            }

            let unit = sqlparser_precision_to_proof_of_sql_time_unit(precision)?;

            Ok(ColumnType::TimestampTZ(unit, PoSQLTimeZone::Utc))
        }
        DataType::Uuid
        | DataType::Binary(_)
        | DataType::Varbinary(_)
        | DataType::Blob(_)
        | DataType::Bytes(_)
        | DataType::Float(_)
        | DataType::MediumInt(_)
        | DataType::UnsignedTinyInt(_)
        | DataType::UnsignedInt2(_)
        | DataType::UnsignedSmallInt(_)
        | DataType::UnsignedMediumInt(_)
        | DataType::UnsignedInt(_)
        | DataType::UnsignedInt4(_)
        | DataType::UnsignedInteger(_)
        | DataType::UnsignedBigInt(_)
        | DataType::UnsignedInt8(_)
        | DataType::Float4
        | DataType::Float64
        | DataType::Real
        | DataType::Float8
        | DataType::Double
        | DataType::DoublePrecision
        | DataType::Date
        | DataType::Time(..)
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
    use super::*;
    use itertools::Itertools;

    #[test]
    fn we_can_convert_simple_postgres_types_to_proof_of_sql() {
        // This test limits itself to types and aliases that appear in postgres documentation

        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::Boolean).unwrap(),
            ColumnType::Boolean,
        );

        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&DataType::TinyInt(None)).unwrap(),
            ColumnType::TinyInt,
        );

        assert_eq!(
            [DataType::SmallInt(None), DataType::Int2(None)]
                .iter()
                .map(sqlparser_data_type_to_proof_of_sql_column_type)
                .map(Result::unwrap)
                .all_equal_value()
                .unwrap(),
            ColumnType::SmallInt
        );

        assert_eq!(
            [
                DataType::Int4(None),
                DataType::Int(None),
                DataType::Integer(None),
            ]
            .iter()
            .map(sqlparser_data_type_to_proof_of_sql_column_type)
            .map(Result::unwrap)
            .all_equal_value()
            .unwrap(),
            ColumnType::Int
        );

        assert_eq!(
            [DataType::Int8(None), DataType::BigInt(None)]
                .iter()
                .map(sqlparser_data_type_to_proof_of_sql_column_type)
                .map(Result::unwrap)
                .all_equal_value()
                .unwrap(),
            ColumnType::BigInt
        );

        assert_eq!(
            [
                DataType::Text,
                DataType::Char(None),
                DataType::Character(None),
                DataType::Varchar(None),
                DataType::CharVarying(None),
                DataType::CharacterVarying(None),
            ]
            .iter()
            .map(sqlparser_data_type_to_proof_of_sql_column_type)
            .map(Result::unwrap)
            .all_equal_value()
            .unwrap(),
            ColumnType::VarChar
        );
    }

    #[test]
    fn we_can_convert_sqlparser_timestamps_to_proof_of_sql() {
        let microsecond_timestamp = DataType::Timestamp(Some(6), TimezoneInfo::WithTimeZone);
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&microsecond_timestamp).unwrap(),
            ColumnType::TimestampTZ(PoSQLTimeUnit::Microsecond, PoSQLTimeZone::Utc)
        );

        let millisecond_timestamp = DataType::Timestamp(Some(3), TimezoneInfo::Tz);
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&millisecond_timestamp).unwrap(),
            ColumnType::TimestampTZ(PoSQLTimeUnit::Millisecond, PoSQLTimeZone::Utc)
        );

        let second_timestamp = DataType::Timestamp(Some(0), TimezoneInfo::WithTimeZone);
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&second_timestamp).unwrap(),
            ColumnType::TimestampTZ(PoSQLTimeUnit::Second, PoSQLTimeZone::Utc)
        );

        let default_timestamp = DataType::Timestamp(None, TimezoneInfo::Tz);
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&default_timestamp).unwrap(),
            ColumnType::TimestampTZ(PoSQLTimeUnit::Microsecond, PoSQLTimeZone::Utc)
        );
    }

    #[test]
    fn we_cannot_convert_sqlparser_timestamps_with_invalid_precision() {
        let decisecond_timestamp = DataType::Timestamp(Some(1), TimezoneInfo::WithTimeZone);
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&decisecond_timestamp),
            Err(UnsupportedColumnType::TimestampPrecision { .. })
        ));

        let nanosecond_timestamp = DataType::Timestamp(Some(9), TimezoneInfo::WithTimeZone);
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&nanosecond_timestamp),
            Err(UnsupportedColumnType::TimestampPrecision { .. })
        ));
    }

    #[test]
    fn we_cannot_convert_sqlparser_timestamps_without_timezone() {
        let timestamp = DataType::Timestamp(None, TimezoneInfo::None);
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&timestamp),
            Err(UnsupportedColumnType::TimestampWithoutTimezone)
        ));

        let timestamp_without_timezone =
            DataType::Timestamp(Some(0), TimezoneInfo::WithoutTimeZone);
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&timestamp_without_timezone),
            Err(UnsupportedColumnType::TimestampWithoutTimezone)
        ));
    }

    #[test]
    fn we_can_convert_sqlparser_decimals_to_proof_of_sql() {
        let full_decimal = DataType::Numeric(ExactNumberInfo::PrecisionAndScale(75, 10));
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&full_decimal).unwrap(),
            ColumnType::Decimal75(Precision::new(75).unwrap(), 10)
        );

        let decimal_with_precision = DataType::Numeric(ExactNumberInfo::Precision(38));
        assert_eq!(
            sqlparser_data_type_to_proof_of_sql_column_type(&decimal_with_precision).unwrap(),
            ColumnType::Decimal75(Precision::new(38).unwrap(), 0)
        );
    }

    #[test]
    fn we_cannot_convert_sqlparser_decimals_without_precision() {
        let unconstrained_decimal = DataType::Numeric(ExactNumberInfo::None);
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&unconstrained_decimal),
            Err(UnsupportedColumnType::UnconstrainedDecimal),
        ));
    }

    #[test]
    fn we_cannot_convert_sqlparser_decimals_with_out_of_bounds_precision() {
        let full_decimal_outside_u8 = DataType::Numeric(ExactNumberInfo::PrecisionAndScale(257, 0));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&full_decimal_outside_u8),
            Err(UnsupportedColumnType::DecimalPrecision { .. }),
        ));

        let decimal_precision_outside_u8 = DataType::Numeric(ExactNumberInfo::Precision(1000));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&decimal_precision_outside_u8),
            Err(UnsupportedColumnType::DecimalPrecision { .. }),
        ));

        let full_decimal_above_75 = DataType::Numeric(ExactNumberInfo::PrecisionAndScale(76, 0));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&full_decimal_above_75),
            Err(UnsupportedColumnType::DecimalPrecision { .. }),
        ));

        let decimal_precision_above_75 = DataType::Numeric(ExactNumberInfo::Precision(100));
        assert!(matches!(
            sqlparser_data_type_to_proof_of_sql_column_type(&decimal_precision_above_75),
            Err(UnsupportedColumnType::DecimalPrecision { .. }),
        ));
    }

    #[test]
    fn we_cannot_convert_sqlparser_decimals_with_out_of_bounds_scale() {
        let decimal_scale_outside_i8 =
            DataType::Numeric(ExactNumberInfo::PrecisionAndScale(75, 128));
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
