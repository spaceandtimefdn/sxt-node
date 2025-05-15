//! Runtime APIs for reading from pallet-commitments.

use alloc::vec::Vec;

use frame_support::BoundedVec;
use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
use proof_of_sql_commitment_map::{AnyCommitmentScheme, TableCommitmentBytes};
use sp_core::ConstU32;
use sxt_core::tables::TableIdentifier;

use super::AnyTableCommitments;

/// The maximum table commitments that can be requested in commitments apis.
pub const MAX_TABLES_IN_TABLE_COMMITMENTS_QUERY: u32 = 64;

/// The maximum table commitments that can be requested in commitments apis as `Get`.
pub type MaxTablesInTableCommitmentsQuery = ConstU32<MAX_TABLES_IN_TABLE_COMMITMENTS_QUERY>;

/// A bounded list of table identifiers used as input in commitments apis.
pub type CommitmentsApiBoundedTableIdentifiersList =
    BoundedVec<TableIdentifier, MaxTablesInTableCommitmentsQuery>;

sp_api::decl_runtime_apis! {
    /// Runtime APIs for reading from pallet-commitments.
    pub trait CommitmentsApi {
        /// Returns the table commitments for the given table identifiers, for the first scheme
        /// that covers all of them.
        ///
        /// Returns `None` if no scheme has complete coverage of the given tables.
        fn table_commitments_any_scheme(table_identifiers: CommitmentsApiBoundedTableIdentifiersList) -> Option<AnyTableCommitments>;
    }
}
