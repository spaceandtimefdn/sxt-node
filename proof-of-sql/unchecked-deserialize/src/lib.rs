//! This crate enables unchecked deserialization for certain types, in particular `TableCommitment<DynamicDoryCommitment>``.
#![no_std]

extern crate alloc;

mod unchecked_dynamic_dory_commitment;
pub use crate::unchecked_dynamic_dory_commitment::UncheckedDynamicDoryCommitment;
mod table_commitment_util;
pub use crate::table_commitment_util::map_table_commitment;
