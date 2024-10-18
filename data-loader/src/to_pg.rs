//! Module provides functions for converting Apache Arrow Flight data to PostgreSQL parameters.
//!
//! The key types include `PgValue` and `PgColumn`, which map Arrow types to
//! PostgreSQL column types and values, enabling bulk inserts from Arrow's `RecordBatch`
//! to PostgreSQL. This is particularly useful for high-volume data pipelines.
//!
//! The conversion includes handling different Arrow types such as Boolean, Int16, Int32,
//! Float64, Utf8 (strings), Timestamps, and Decimal types. Each type is converted to a corresponding
//! PostgreSQL-compatible type.

use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;

use arrow::array::{
    Array,
    BinaryArray,
    BooleanArray,
    Date32Array,
    Decimal128Array,
    FixedSizeBinaryArray,
    Float32Array,
    Float64Array,
    Int16Array,
    Int32Array,
    Int64Array,
    StringArray,
    Time64MicrosecondArray,
    TimestampMicrosecondArray,
    TimestampMillisecondArray,
    UInt32Array,
};
use arrow::datatypes::{DataType, TimeUnit};
use arrow::record_batch::RecordBatch;
use arrow_array::TimestampNanosecondArray;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use pg_bigdecimal::{BigDecimal, PgNumeric};
use rust_decimal::Decimal;
use tokio_postgres::types::private::BytesMut;
use tokio_postgres::types::{IsNull, ToSql, Type as PostgresType};
use tonic::Status;

use crate::data_loader::META_ROW_NUMBER_COLUMN_NAME;
use crate::{err, status};

/// `PgColumn` represents the metadata for a PostgreSQL column including its name,
/// data type, and optional numeric precision/scale for decimal types.
#[derive(Debug)]
pub struct PgColumn {
    /// The name of the column in the PostgreSQL database.
    /// This field is a `String` and should match the column name in the table.
    pub column_name: String,

    /// The data type of the column as a `String`. This reflects the SQL data type
    /// of the column, such as `INTEGER`, `VARCHAR`, `NUMERIC`, etc.
    pub data_type: String,

    /// Optional field that specifies the numeric precision of the column if the
    /// column's data type is `NUMERIC`. Precision refers to the total number
    /// of digits that can be stored in the column.
    /// If the data type is not numeric, this field is `None`.
    pub numeric_precision: Option<i32>,

    /// Optional field that specifies the numeric scale of the column if the
    /// column's data type is `NUMERIC`. Scale refers to the number of digits
    /// to the right of the decimal point.
    /// If the data type is not numeric, this field is `None`.
    pub numeric_scale: Option<i32>,
}

/// `PgValue` represents various data types that can be used as parameters
/// in PostgreSQL queries. Each variant corresponds to a specific data type
/// that PostgreSQL supports.
#[derive(Clone, Debug)]
pub enum PgValue {
    /// Represents a `bool` value, corresponding to PostgreSQL's `BOOLEAN` type.
    Boolean(bool),

    /// Represents a 16-bit signed integer (`i16`), corresponding to PostgreSQL's `SMALLINT` type.
    Int16(i16),

    /// Represents a 32-bit signed integer (`i32`), corresponding to PostgreSQL's `INTEGER` type.
    Int32(i32),

    /// Represents a 64-bit signed integer (`i64`), corresponding to PostgreSQL's `BIGINT` type.
    Int64(i64),

    /// Represents a 32-bit unsigned integer (`u32`), typically used for PostgreSQL's `OID` (Object Identifier) type.
    UInt32(u32),

    /// Represents a 32-bit floating-point number (`f32`), corresponding to PostgreSQL's `REAL` type.
    Float32(f32),

    /// Represents a 64-bit floating-point number (`f64`), corresponding to PostgreSQL's `DOUBLE PRECISION` type.
    Float64(f64),

    /// Represents a `String` value, corresponding to PostgreSQL's `TEXT` or `VARCHAR` type.
    Text(String),

    /// Represents a `Vec<u8>`, corresponding to PostgreSQL's `BYTEA` type for storing binary data.
    Binary(Vec<u8>),

    /// Represents a `Decimal` value, corresponding to PostgreSQL's `NUMERIC` type for high-precision numbers.
    Numeric(Decimal),

    /// Represents a `PgNumeric` (from the `pg_bigdecimal` crate), a custom type used for handling
    /// high-precision numbers in PostgreSQL's `NUMERIC` type with arbitrary precision.
    BigDecimal(PgNumeric),

    /// Represents a `NaiveDateTime` (from the `chrono` crate), corresponding to PostgreSQL's `TIMESTAMP` type,
    /// which stores date and time without time zone information.
    Timestamp(NaiveDateTime),

    /// Represents a `DateTime<Utc>` (from the `chrono` crate), corresponding to PostgreSQL's `TIMESTAMPTZ` type,
    /// which stores date and time with time zone information.
    TimestampTz(DateTime<Utc>),

    /// Represents a `NaiveDate` (from the `chrono` crate), corresponding to PostgreSQL's `DATE` type.
    Date(NaiveDate),

    /// Represents a `NaiveTime` (from the `chrono` crate), corresponding to PostgreSQL's `TIME` type,
    /// which stores just the time of day (without a date).
    Time(NaiveTime),

    /// Represents a null value in PG column
    Null,
}

impl ToSql for PgValue {
    /// Converts the `PgValue` enum into a format suitable for PostgreSQL,
    /// using the appropriate type for each variant.
    ///
    /// This method implements `ToSql` for various types (e.g., `Int32`, `Float64`, `Text`).
    /// It writes the value to the provided `BytesMut` buffer.
    fn to_sql(
        &self,
        ty: &PostgresType,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        match self {
            PgValue::Boolean(v) => v.to_sql(ty, out),
            PgValue::Int16(v) => v.to_sql(ty, out),
            PgValue::Int32(v) => v.to_sql(ty, out),
            PgValue::Int64(v) => v.to_sql(ty, out),
            PgValue::UInt32(v) => v.to_sql(ty, out),
            PgValue::Float32(v) => v.to_sql(ty, out),
            PgValue::Float64(v) => v.to_sql(ty, out),
            PgValue::Text(v) => v.to_sql(ty, out),
            PgValue::Binary(v) => v.to_sql(ty, out),
            PgValue::Numeric(v) => v.to_sql(ty, out),
            PgValue::Timestamp(v) => v.to_sql(ty, out),
            PgValue::TimestampTz(v) => v.to_sql(ty, out),
            PgValue::Date(v) => v.to_sql(ty, out),
            PgValue::Time(v) => v.to_sql(ty, out),
            PgValue::BigDecimal(v) => v.to_sql(ty, out),
            PgValue::Null => Ok(IsNull::Yes), // Handle NULL case
        }
    }

    /// Indicates which PostgreSQL types this `PgValue` supports for conversion.
    fn accepts(ty: &PostgresType) -> bool {
        matches!(
            ty,
            &PostgresType::BOOL
                | &PostgresType::INT2
                | &PostgresType::INT4
                | &PostgresType::INT8
                | &PostgresType::OID
                | &PostgresType::FLOAT4
                | &PostgresType::FLOAT8
                | &PostgresType::TEXT
                | &PostgresType::BYTEA
                | &PostgresType::NUMERIC
                | &PostgresType::TIMESTAMP
                | &PostgresType::TIMESTAMPTZ
                | &PostgresType::DATE
                | &PostgresType::TIME
        )
    }

    /// Performs a conversion check before writing the `PgValue` into the provided buffer.
    fn to_sql_checked(
        &self,
        ty: &PostgresType,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        self.to_sql(ty, out)
    }
}

/// Converts an Arrow `RecordBatch` into PostgreSQL-compatible values for insertion.
///
/// # Arguments
///
/// * `param_schema` - A reference to the schema of the `RecordBatch` columns.
/// * `rb` - The `RecordBatch` containing Arrow arrays.
/// * `index` - The row index to extract values from.
/// * `column_map` - A map of column names to PostgreSQL metadata.
///
/// # Returns
///
/// Returns a `Vec<PgValue>` representing the row at the given index in the `RecordBatch`.
pub fn get_pg_values(
    rb: &RecordBatch,
    index: usize,
    column_map: &HashMap<String, PgColumn>,
) -> Result<Vec<PgValue>, Status> {
    let param_schema = rb.schema();
    let values: Vec<PgValue> = rb
        .columns()
        .iter()
        .zip(param_schema.fields().iter())
        .map(|(col, field)| match field.data_type() {
            DataType::Boolean => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::Boolean(src_col.value(index)))
                }
            }
            DataType::Int16 => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<Int16Array>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::Int16(src_col.value(index)))
                }
            }
            DataType::Int32 => {
                // Hack alert!! Specially handled for META_ROW_NUMBER
                let src_col = col
                    .as_any()
                    .downcast_ref::<Int32Array>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let col_name = field.name().to_uppercase();
                    if col_name.eq(META_ROW_NUMBER_COLUMN_NAME) {
                        Ok(PgValue::Int64(src_col.value(index) as i64))
                    } else {
                        Ok(PgValue::Int32(src_col.value(index)))
                    }
                }
            }
            DataType::Int64 => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::Int64(src_col.value(index)))
                }
            }
            DataType::UInt32 => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<UInt32Array>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::UInt32(src_col.value(index)))
                }
            }
            DataType::Float32 => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<Float32Array>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::Float32(src_col.value(index)))
                }
            }
            DataType::Float64 => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::Float64(src_col.value(index)))
                }
            }
            DataType::Utf8 => {
                let col_name = field.name().to_lowercase();
                let d = column_map.get(&col_name);
                if let Some(data) = d {
                    if data.data_type.eq_ignore_ascii_case("numeric") {
                        let src_col = col
                            .as_any()
                            .downcast_ref::<StringArray>()
                            .ok_or_else(|| status!(err!("Types don't match")))?;

                        if src_col.is_null(index) {
                            return Ok(PgValue::Null);
                        } else {
                            let decimal_str = src_col.value(index);
                            let decimal = if decimal_str.contains("e") || decimal_str.contains("E")
                            {
                                let decimal_format_str = BigDecimal::from_str(src_col.value(index))
                                    .unwrap()
                                    .to_string();
                                Some(BigDecimal::from_str(&decimal_format_str).unwrap())
                            } else {
                                Some(BigDecimal::from_str(src_col.value(index)).unwrap())
                            };
                            return Ok(PgValue::BigDecimal(PgNumeric::new(decimal)));
                        }
                    }
                }
                let src_col = col
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    // Hack Alert !! to clean up null bytes which are not supported by utf8
                    let input_data = src_col.value(index).to_string();
                    let clean_data = input_data.replace("\0", ""); // Removes null bytes
                    Ok(PgValue::Text(clean_data))
                }
            }
            DataType::Binary => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::Binary(src_col.value(index).to_vec()))
                }
            }
            DataType::FixedSizeBinary(_) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<FixedSizeBinaryArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::Binary(src_col.value(index).to_vec()))
                }
            }
            DataType::Decimal128(_p, s) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<Decimal128Array>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let decimal = Decimal::from_i128_with_scale(src_col.value(index), *s as u32);
                    Ok(PgValue::Numeric(decimal))
                }
            }
            DataType::Timestamp(TimeUnit::Microsecond, None) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<TimestampMicrosecondArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let timestamp = DateTime::from_timestamp_micros(src_col.value(index))
                        .ok_or_else(|| status!(err!("Invalid timestamp value")))?
                        .naive_utc();
                    Ok(PgValue::Timestamp(timestamp))
                }
            }
            DataType::Timestamp(TimeUnit::Microsecond, Some(_)) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<TimestampMicrosecondArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                let timestamp = DateTime::<Utc>::from_timestamp_micros(src_col.value(index))
                    .ok_or_else(|| status!(err!("Invalid timestamp value")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    Ok(PgValue::TimestampTz(timestamp))
                }
            }
            DataType::Timestamp(TimeUnit::Millisecond, None) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<TimestampMillisecondArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let datetime = DateTime::<Utc>::from_timestamp_millis(src_col.value(index))
                        .ok_or_else(|| status!(err!("Invalid timestamp value")))?
                        .naive_utc();
                    Ok(PgValue::Timestamp(datetime))
                }
            }
            DataType::Timestamp(TimeUnit::Millisecond, Some(_)) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<TimestampMillisecondArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let datetime = DateTime::<Utc>::from_timestamp_millis(src_col.value(index))
                        .ok_or_else(|| status!(err!("Invalid timestamp value")))?;
                    Ok(PgValue::TimestampTz(datetime))
                }
            }

            DataType::Timestamp(TimeUnit::Nanosecond, None) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<TimestampNanosecondArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let datetime =
                        DateTime::<Utc>::from_timestamp_nanos(src_col.value(index)).naive_utc();
                    Ok(PgValue::Timestamp(datetime))
                }
            }
            DataType::Timestamp(TimeUnit::Nanosecond, Some(_)) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<TimestampNanosecondArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let datetime =
                        DateTime::<Utc>::from_timestamp_nanos(src_col.value(index)).naive_utc();
                    Ok(PgValue::Timestamp(datetime))
                }
            }
            DataType::Date32 => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<Date32Array>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let date = NaiveDate::from_num_days_from_ce_opt(src_col.value(index))
                        .ok_or_else(|| status!(err!("Invalid date value")))?;
                    Ok(PgValue::Date(date))
                }
            }
            DataType::Time64(TimeUnit::Microsecond) => {
                let src_col = col
                    .as_any()
                    .downcast_ref::<Time64MicrosecondArray>()
                    .ok_or_else(|| status!(err!("Types don't match")))?;
                if src_col.is_null(index) {
                    Ok(PgValue::Null)
                } else {
                    let time = NaiveTime::from_num_seconds_from_midnight_opt(
                        (src_col.value(index) / 1_000_000) as u32,
                        ((src_col.value(index) % 1_000_000) * 1000) as u32,
                    )
                    .ok_or_else(|| status!(err!("Invalid time value")))?;
                    Ok(PgValue::Time(time))
                }
            }
            _ => Err(status!(err!(
                "Unsupported parameter type: {}",
                field.data_type()
            ))),
        })
        .collect::<Result<Vec<_>, Status>>()?;

    Ok(values)
}

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use pg_bigdecimal::{BigDecimal, PgNumeric};
    use tokio_postgres::types::private::BytesMut;
    use tokio_postgres::types::{ToSql, Type as PostgresType};

    #[tokio::test]
    async fn test_get_pg_values() {
        let val = "10e75";
        let val1 = convert_exponent_to_decimal_str(val).unwrap();

        println!("convert {}", convert_exponent_to_decimal_str(val).unwrap());
        let decimal = Some(BigDecimal::from_str(&val1).unwrap());

        let decimal_x = PgNumeric::new(decimal);
        let mut x = BytesMut::new();
        decimal_x.to_sql(&PostgresType::NUMERIC, &mut x).unwrap();
    }

    fn convert_exponent_to_decimal_str(input: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Parse the input string into a BigDecimal
        let big_decimal = BigDecimal::from_str(input)?;
        Ok(big_decimal.to_string())
    }
}
