//! Module containing `WEIGHT_FEE` and `TARGET_BYTE_FEE`.

/// We want to base our pricing on the cost of the data insertion transaction since this is the
/// most common action on the network. The values below are intended to represent an 'Average'
/// Insert of 256 bytes of data per row.
const AVERAGE_INSERT_SIZE_BYTES_PER_ROW: u128 = 256;
const AVERAGE_INSERT_TARGET_COST_PER_ROW: u128 = 10_000_000_000_000u128.saturating_mul(20);
pub const TARGET_BYTE_FEE: u128 =
    AVERAGE_INSERT_TARGET_COST_PER_ROW.saturating_div(AVERAGE_INSERT_SIZE_BYTES_PER_ROW);

/// This value should be the coefficient of a 1-element-insert, as measured in pallet-indexing's weights.rs
const INSERT_CALL_WEIGHT_PER_ELEMENT: u128 = 33_837_719;
/// The number of elements per row that the target cost should apply to
const INSERT_FEE_TARGET_ROW_LENGTH: u128 = 16;

/// Insert weight for an insert with INSERT_FEE_TARGET_ROW_COUNT rows.
const INSERT_FEE_TARGET_CALL_WEIGHT: u128 =
    INSERT_CALL_WEIGHT_PER_ELEMENT.saturating_mul(INSERT_FEE_TARGET_ROW_LENGTH);
pub const WEIGHT_FEE: u128 =
    AVERAGE_INSERT_TARGET_COST_PER_ROW.saturating_div(INSERT_FEE_TARGET_CALL_WEIGHT);
