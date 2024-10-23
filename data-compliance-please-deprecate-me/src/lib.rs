#![doc = include_str!("../README.md")]

mod null_bytes;
pub use null_bytes::column_remove_null_bytes;

mod nulls;
pub use nulls::{column_def_not_null, column_default_nulls};

mod decimal_precision;
pub use decimal_precision::{column_clamp_precision, column_def_clamp_precision};

mod parse_decimals;
pub use parse_decimals::{
    column_parse_decimals_fallible,
    column_parse_decimals_unchecked,
    ParseDecimalsError,
};

mod record_batch_map;
pub use record_batch_map::{
    record_batch_map,
    record_batch_map_with_target_types,
    record_batch_try_map_with_target_types,
    MapOrTargetTypeError,
    TargetTypeNotFound,
};
