//! This module holds constants related to fees and their calculations on-chain

/// Balance of an account.
pub type Balance = u128;

/// A single SXT
pub const UNITS: Balance = 1_000_000_000_000_000_000;

/// A convenient alias
pub const DOLLARS: Balance = UNITS; // 10_000_000_000

/// 1000 SXT
pub const GRAND: Balance = DOLLARS * 1_000; // 10_000_000_000_000

/// 0.01 SXT
pub const CENTS: Balance = DOLLARS / 100; // 100_000_000

/// 0.0001 SXT
pub const MILLICENTS: Balance = CENTS / 1_000; // 100_000

/// We want to base our pricing on the cost of the data insertion transaction since this is the
/// most common action on the network. The values below are intended to represent an 'Average'
/// Insert of 5000 bytes of data.
pub const CALIBRATION_MULTIPLIER: u128 = 43; // A Calibration multiplier to reach the desired target pricing

/// The average size of an indexer insert
pub const AVERAGE_INSERT_SIZE_BYTES: u128 = 8192;

/// The cost target for an average insert
pub const AVERAGE_INSERT_TARGET_COST: u128 = MILLICENTS
    .saturating_mul(20)
    .saturating_mul(CALIBRATION_MULTIPLIER);
/// The fee we should charge per byte of data
pub const TARGET_BYTE_FEE: u128 =
    AVERAGE_INSERT_TARGET_COST.saturating_div(AVERAGE_INSERT_SIZE_BYTES);
/// Approximated Average Insert Weight from actual transactions on testnet
pub const AVERAGE_INSERT_CALL_WEIGHT: u128 = 7_582_873_000;
/// The fee we should charge per unit of Weight. This is the 'big knob'
pub const WEIGHT_FEE: u128 = AVERAGE_INSERT_TARGET_COST.saturating_div(AVERAGE_INSERT_CALL_WEIGHT);
