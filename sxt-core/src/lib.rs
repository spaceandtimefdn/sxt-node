#![cfg_attr(not(feature = "std"), no_std)]

use sp_core::ConstU32;
use sp_runtime::BoundedVec;

/// Types and functionality related to the permissions system
pub mod permissions;

/// Types and functionality realt
pub mod tables;

const IDENT_LENGTH: u32 = 64;

/// The maximum length of u8 strings in the system
pub type IdentLength = ConstU32<IDENT_LENGTH>;

/// A IdentLength length bounded vector of u8 data, represents the basic string type on the substrate side.
pub type ByteString = BoundedVec<u8, IdentLength>;
