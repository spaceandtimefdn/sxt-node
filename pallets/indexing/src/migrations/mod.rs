//! Storage migrations for the indexing pallet

pub mod v1;

pub mod v2;

/// A unique identifier across all pallets.
///
/// This constant represents a unique identifier for the migrations of this pallet.
/// It helps differentiate migrations for this pallet from those of others. Note that we don't
/// directly pull the crate name from the environment, since that would change if the crate were
/// ever to be renamed and could cause historic migrations to run again.
pub const PALLET_MIGRATIONS_ID: &[u8; 26] = b"pallet-indexing-migrations";
