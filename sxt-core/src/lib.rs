//! TODO: add docs
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use sp_core::ConstU32;
use sp_runtime::BoundedVec;

/// Types and functionality related to the permissions system
pub mod permissions;

/// Types and functionality related to tables
pub mod tables;

/// Types primarily used by the indexing pallet for data submissions and quorum finding
pub mod indexing;

/// Types consumed by the native code interface
pub mod native;

/// A module for parsing DDLs into table create statements
/// Used in building the genesis chain spec
/// Enabled with the 'std' feature
#[cfg(feature="std")]
pub mod parsing;

/// The maximum length of Identifiers
const IDENT_LENGTH: u32 = 64;

/// The maximum length of u8 strings in the system
pub type IdentLength = ConstU32<IDENT_LENGTH>;

/// A IdentLength length bounded vector of u8 data, represents the basic string type on the substrate side.
pub type ByteString = BoundedVec<u8, IdentLength>;
