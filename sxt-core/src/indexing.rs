use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::BoundedVec;
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::{ConstU32, TypeInfo};
use sp_core::U256;
use sp_runtime::BoundedBTreeSet;

use crate::tables::{QuorumScope, TableIdentifier, TableUuid};

/// Maximum length of submitted Record Batch Data
pub const DATA_MAX_LEN: u32 = 8_000_000;
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
pub type SubmitterList<T> = BoundedBTreeSet<T, ConstU32<MAX_SUBMITTERS>>;

/// Lists of agreeing submitters per quorum scope.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct SubmittersByScope<T> {
    /// Agreeing submitters approaching public quorum.
    public: SubmitterList<T>,
    /// Agreeing submitters approaching privileged quorum.
    privileged: SubmitterList<T>,
}

// Manual implementation of Default doesn't require that `T: Default`.
impl<T: Ord> Default for SubmittersByScope<T> {
    fn default() -> Self {
        SubmittersByScope {
            public: Default::default(),
            privileged: Default::default(),
        }
    }
}

impl<T: Ord> SubmittersByScope<T> {
    /// Returns the submitter list of the given quorum scope.
    pub fn of_scope(&self, quorum_scope: &QuorumScope) -> &SubmitterList<T> {
        match quorum_scope {
            QuorumScope::Public => &self.public,
            QuorumScope::Privileged => &self.privileged,
        }
    }

    /// Returns `self` with an additional entry for the given submitter in the given quorum scope.
    ///
    /// If the submitter already exists in the list, self is returned silently.
    ///
    /// ## Errors
    /// Fails if the resulting list would exceed [`MAX_SUBMITTERS`].
    /// In this case, the method returns a pair containing `self` unchanged and the new submitter.
    pub fn with_submitter(
        mut self,
        submitter: T,
        quorum_scope: &QuorumScope,
    ) -> Result<Self, (Self, T)> {
        let try_insert_result = match quorum_scope {
            QuorumScope::Public => self.public.try_insert(submitter),
            QuorumScope::Privileged => self.privileged.try_insert(submitter),
        };

        // more difficult to handle with map/map_err without cloning self
        match try_insert_result {
            Ok(_) => Ok(self),
            Err(e) => Err((self, e)),
        }
    }

    /// Returns the length of the submitter list of the given quorum scope.
    pub fn len_of_scope(&self, quorum_scope: &QuorumScope) -> usize {
        match quorum_scope {
            QuorumScope::Public => self.public.len(),
            QuorumScope::Privileged => self.privileged.len(),
        }
    }

    /// Returns true if the length of the submitter list of the given quorum scope is 0.
    pub fn scope_is_empty(&self, quorum_scope: &QuorumScope) -> bool {
        self.len_of_scope(quorum_scope) == 0
    }

    /// Consumes `self` and returns an iterator over the submitters of the given quorum scope.
    pub fn into_iter_scope(self, quorum_scope: &QuorumScope) -> impl Iterator<Item = T> {
        match quorum_scope {
            QuorumScope::Public => self.public.into_iter(),
            QuorumScope::Privileged => self.privileged.into_iter(),
        }
    }

    /// Returns a borrowing iterator over the submitters of the given quorum scope.
    pub fn iter_scope(&self, quorum_scope: &QuorumScope) -> impl Iterator<Item = &T> {
        match quorum_scope {
            QuorumScope::Public => self.public.iter(),
            QuorumScope::Privileged => self.privileged.iter(),
        }
    }
}

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

    /// Quorum scope this submission contributed to.
    pub quorum_scope: QuorumScope,
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

    /// Quorum scope that reached quorum.
    pub quorum_scope: QuorumScope,
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeSet;
    use alloc::vec;

    use super::*;

    #[test]
    fn we_can_read_empty_submitters_by_scope() {
        let empty_submitters = SubmittersByScope::<u32>::default();

        assert_eq!(
            empty_submitters.of_scope(&QuorumScope::Public),
            &SubmitterList::default()
        );
        assert_eq!(
            empty_submitters.of_scope(&QuorumScope::Privileged),
            &SubmitterList::default()
        );
        assert_eq!(empty_submitters.len_of_scope(&QuorumScope::Public), 0);
        assert_eq!(empty_submitters.len_of_scope(&QuorumScope::Privileged), 0);
        assert!(empty_submitters.scope_is_empty(&QuorumScope::Public));
        assert!(empty_submitters.scope_is_empty(&QuorumScope::Privileged));
        assert_eq!(
            empty_submitters
                .iter_scope(&QuorumScope::Public)
                .collect::<Vec<_>>(),
            Vec::<&u32>::new()
        );
        assert_eq!(
            empty_submitters
                .iter_scope(&QuorumScope::Privileged)
                .collect::<Vec<_>>(),
            Vec::<&u32>::new()
        );
        assert_eq!(
            empty_submitters
                .clone()
                .into_iter_scope(&QuorumScope::Public)
                .collect::<Vec<_>>(),
            Vec::<u32>::new()
        );
        assert_eq!(
            empty_submitters
                .into_iter_scope(&QuorumScope::Privileged)
                .collect::<Vec<_>>(),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn we_can_read_populated_submitters_by_scope() {
        let public = BoundedBTreeSet::try_from(BTreeSet::from_iter([0, 2])).unwrap();
        let privileged = BoundedBTreeSet::try_from(BTreeSet::from_iter([1])).unwrap();

        let submitters = SubmittersByScope::<u32> {
            public: public.clone(),
            privileged: privileged.clone(),
        };

        assert_eq!(submitters.of_scope(&QuorumScope::Public), &public);
        assert_eq!(submitters.of_scope(&QuorumScope::Privileged), &privileged);
        assert_eq!(submitters.len_of_scope(&QuorumScope::Public), 2);
        assert_eq!(submitters.len_of_scope(&QuorumScope::Privileged), 1);
        assert!(!submitters.scope_is_empty(&QuorumScope::Public));
        assert!(!submitters.scope_is_empty(&QuorumScope::Privileged));
        assert_eq!(
            submitters
                .iter_scope(&QuorumScope::Public)
                .collect::<Vec<_>>(),
            vec![&0, &2]
        );
        assert_eq!(
            submitters
                .iter_scope(&QuorumScope::Privileged)
                .collect::<Vec<_>>(),
            vec![&1]
        );
        assert_eq!(
            submitters
                .clone()
                .into_iter_scope(&QuorumScope::Public)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(
            submitters
                .into_iter_scope(&QuorumScope::Privileged)
                .collect::<Vec<_>>(),
            vec![1]
        );
    }

    #[test]
    fn we_can_construct_submitters_by_scope_with_additional_submitter() {
        let submitters = SubmittersByScope::<u32>::default();

        let submitters = submitters
            .with_submitter(10, &QuorumScope::Privileged)
            .unwrap();
        assert_eq!(
            submitters,
            SubmittersByScope {
                public: Default::default(),
                privileged: BTreeSet::from_iter([10]).try_into().unwrap()
            }
        );

        let submitters = submitters.with_submitter(11, &QuorumScope::Public).unwrap();
        assert_eq!(
            submitters,
            SubmittersByScope {
                public: BTreeSet::from_iter([11]).try_into().unwrap(),
                privileged: BTreeSet::from_iter([10]).try_into().unwrap()
            }
        );

        let submitters = submitters
            .with_submitter(11, &QuorumScope::Privileged)
            .unwrap();
        let expected_submitters = SubmittersByScope {
            public: BTreeSet::from_iter([11]).try_into().unwrap(),
            privileged: BTreeSet::from_iter([10, 11]).try_into().unwrap(),
        };
        assert_eq!(submitters, expected_submitters,);

        // adding already-existing submitter does nothing
        let submitters = submitters
            .with_submitter(10, &QuorumScope::Privileged)
            .unwrap();
        assert_eq!(submitters, expected_submitters);
    }

    #[test]
    fn we_cannot_add_submitter_exceeding_maximum() {
        let submitters = SubmittersByScope::<u32> {
            public: BTreeSet::from_iter(0..MAX_SUBMITTERS).try_into().unwrap(),
            privileged: Default::default(),
        };
        let expected_submitters = submitters.clone();

        let (submitters, item) = submitters
            .with_submitter(MAX_SUBMITTERS, &QuorumScope::Public)
            .unwrap_err();

        assert_eq!(submitters, expected_submitters);
        assert_eq!(item, MAX_SUBMITTERS);

        // we can still add to the other scope
        let submitters = submitters
            .with_submitter(MAX_SUBMITTERS, &QuorumScope::Privileged)
            .unwrap();
        let expected_submitters = SubmittersByScope {
            privileged: BTreeSet::from_iter([MAX_SUBMITTERS]).try_into().unwrap(),
            ..expected_submitters
        };

        assert_eq!(submitters, expected_submitters);
    }
}
