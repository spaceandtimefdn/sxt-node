use crate::tables::TableIdentifier;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::BoundedVec;
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::{ConstU32, TypeInfo};
use sp_core::U256;

/// Maximum length of submitted Record Batch Data
pub const DATA_MAX_LEN: u32 = 2_000_000;
/// Used to represent submitted data in it's serialized RecordBatch IPC format
pub type RowData = BoundedVec<u8, ConstU32<DATA_MAX_LEN>>;

/// Our block number
pub type BlockNumber = U256;
/// The maximum length of a batch id
pub const ID_LEN: u32 = 36;
/// Used to represent a batch id for a given submission
pub type BatchId = BoundedVec<u8, ConstU32<ID_LEN>>;

/// The maximum number of submitters for a particular batch id
pub const MAX_SUBMITTERS: u32 = 32;
/// A list of submitter account IDs, We use the generic to allow us to use the runtime's
/// accountId, regardless of the underlying implementation of that Id
pub type SubmitterList<T> = BoundedVec<T, ConstU32<MAX_SUBMITTERS>>;

/// This struct is used to represent all relevant data from an indexing submission
/// when emitting an event
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DataSubmission<Hash> {
    /// The Table Identifier
    pub table: TableIdentifier,

    /// A unique string that represents a new batch
    pub batch_id: BatchId,

    /// The Hash of the submitted data
    pub data_hash: Hash,
}

/// Once the network has received enough submissions for a given BatchId, we will
/// identify the submission data with the majority of submissions and come to a quorum. This
/// struct is used to record data needed for verifying the quorum and issuing rewards or penalties
/// to participants.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct DataQuorum<AccountId, Hash> {
    /// The table identifier of the destination table
    pub table: TableIdentifier,

    /// Batch Id for this data batch
    pub batch_id: BatchId,

    /// The hash of the data upon which we've decided
    pub data_hash: Hash,

    /// The block number of when the quorum was reached
    pub block_number: BlockNumber,

    /// List of account ids that submitted the same data for this batch
    pub agreements: SubmitterList<AccountId>,

    /// List of account ids that submitted different data for this batch
    pub dissents: SubmitterList<AccountId>,
}
