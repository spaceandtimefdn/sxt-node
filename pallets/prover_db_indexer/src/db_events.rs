//! Read events straight from the node's client (via the `ClientExt` runtime interface
//! from `native::client`) instead of replaying the `prover_db_indexer/event` offchain-DB
//! entries written by `EventCapture`.

use alloc::string::String;
use alloc::vec::Vec;
use core::convert::Infallible;
use core::marker::PhantomData;

use codec::Decode;
use pallet_tables::UpdateTableList;
use polkadot_sdk::frame_support::pallet_prelude::ConstU32;
use polkadot_sdk::frame_support::{storage_alias, BoundedVec};
use polkadot_sdk::frame_system;
use polkadot_sdk::sp_core::H256;
use sxt_core::indexing::{DataQuorum, DATA_MAX_LEN};
use sxt_core::tables::{Source, TableIdentifier, TableType};

/// Errors that can occur when querying the node's client for captured events.
#[derive(Debug, snafu::Snafu)]
pub enum DBEventError {
    /// This error occurs when the node's client is not available to query for events.
    #[snafu(display("no client available to query System::Events"))]
    NoClient,
    /// This error occurs when the node's client is available, but the query for events fails.
    #[snafu(display("failed to query System::Events: {message}"))]
    Lookup {
        /// The error message returned by the node's client when the query for events fails.
        message: String,
    },
    /// This error occurs when the node's client is available, but the query for events returns an empty result.
    #[snafu(display("System::Events is empty at this block"))]
    Empty,
    /// This error occurs when the node's client is available, but the query for events returns a result that cannot be decoded.
    #[snafu(display("failed to decode System::Events: {error}"))]
    Decode {
        /// The error returned by the codec when decoding the result of the query for events fails.
        error: codec::Error,
    },
}

type RuntimeEvent<T> = <T as frame_system::Config>::RuntimeEvent;
type EventRecord<T> = frame_system::EventRecord<RuntimeEvent<T>, <T as frame_system::Config>::Hash>;

#[storage_alias]
type Events<T: frame_system::Config> = StorageValue<frame_system::Pallet<T>, Vec<EventRecord<T>>>;

/// A captured event from the node's client
pub enum DBEvent<T: pallet_tables::Config + pallet_indexing::Config<I>, I: 'static = ()> {
    /// Table definitions have been updated.
    SchemaUpdated(Option<T::AccountId>, UpdateTableList),
    /// A table has been successfully dropped.
    TableDropped(Option<T::AccountId>, TableType, TableIdentifier, Source),
    /// This event is emitted when a quorum is reached amongst submissions and the
    /// data is finalized.
    QuorumReached {
        /// The quorum object representing the metadata about the decision
        quorum: DataQuorum<T::AccountId, T::Hash>,
        /// The finalized raw data in postcard serialized OnChainTable bytes
        data: BoundedVec<u8, ConstU32<DATA_MAX_LEN>>,
    },
    /// Never constructed; carries the `pallet_indexing` instance this `DBEvent` was decoded
    /// against, since none of the variants above otherwise name it.
    #[doc(hidden)]
    _Instance(PhantomData<I>, Infallible),
}

impl<T, I> TryFrom<EventRecord<T>> for DBEvent<T, I>
where
    T: pallet_tables::Config + pallet_indexing::Config<I>,
    I: 'static,
    RuntimeEvent<T>: TryInto<pallet_tables::Event<T>> + TryInto<pallet_indexing::Event<T, I>>,
{
    type Error = ();

    fn try_from(record: EventRecord<T>) -> Result<Self, Self::Error> {
        if let Ok(event) = record.event.clone().try_into() {
            match event {
                pallet_tables::Event::SchemaUpdated(owner, tables) => {
                    Ok(DBEvent::SchemaUpdated(owner, tables))
                }
                pallet_tables::Event::TableDropped(owner, table_type, table, source) => {
                    Ok(DBEvent::TableDropped(owner, table_type, table, source))
                }
                _ => Err(()),
            }
        } else if let Ok(pallet_indexing::Event::QuorumReached { quorum, data }) =
            record.event.try_into()
        {
            Ok(DBEvent::QuorumReached { quorum, data })
        } else {
            Err(())
        }
    }
}

/// Queries the node's client for captured events at the given block hash.
pub fn db_events_at<T, I: 'static>(
    block_hash: H256,
) -> Result<impl Iterator<Item = DBEvent<T, I>>, DBEventError>
where
    T: pallet_tables::Config + pallet_indexing::Config<I>,
    EventRecord<T>: TryInto<DBEvent<T, I>>,
{
    let raw = native::client::client::storage(block_hash, Events::<T>::hashed_key().to_vec())
        .ok_or(DBEventError::NoClient)?
        .map_err(|message| DBEventError::Lookup { message })?
        .ok_or(DBEventError::Empty)?;

    Ok(Vec::<EventRecord<T>>::decode(&mut raw.0.as_slice())
        .map_err(|error| DBEventError::Decode { error })?
        .into_iter()
        .filter_map(|record| record.try_into().ok()))
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::sync::Arc;

    use codec::Encode;
    use native::client::ClientExt;
    use native_api::Api;
    use pallet_tables::UpdateTableList;
    use polkadot_sdk::frame_support::BoundedVec;
    use polkadot_sdk::frame_system::Phase;
    use polkadot_sdk::sp_core::storage::StorageData;
    use polkadot_sdk::sp_core::H256;
    use polkadot_sdk::sp_runtime::AccountId32;
    use polkadot_sdk::{frame_system, pallet_balances};
    use sxt_core::indexing::{BatchId, DataQuorum, SubmitterList};
    use sxt_core::tables::{QuorumScope, Source, TableIdentifier, TableType};

    use super::{db_events_at, DBEvent, DBEventError, EventRecord};
    use crate::mock::{new_test_ext, MockClientProvider, RuntimeEvent, Test};

    fn event_record(event: impl Into<RuntimeEvent>) -> EventRecord<Test> {
        frame_system::EventRecord {
            phase: Phase::Initialization,
            event: event.into(),
            topics: vec![],
        }
    }

    #[test]
    fn no_client_when_extension_not_registered() {
        let mut ext = new_test_ext();
        ext.execute_with(|| {
            assert!(matches!(
                db_events_at::<Test, Api>(H256::zero()),
                Err(DBEventError::NoClient)
            ));
        });
    }

    #[test]
    fn lookup_error_is_forwarded() {
        let mut ext = new_test_ext();
        ext.register_extension(ClientExt(Arc::new(MockClientProvider {
            finalized_state: None,
            storage_response: Err("test error".to_string()),
        })));
        ext.execute_with(|| {
            let err = db_events_at::<Test, Api>(H256::zero()).err().unwrap();
            assert!(
                matches!(err, DBEventError::Lookup { message } if message == "UnknownBlock: test error")
            );
        });
    }

    #[test]
    fn empty_when_no_value_at_key() {
        let mut ext = new_test_ext();
        ext.register_extension(ClientExt(Arc::new(MockClientProvider {
            finalized_state: None,
            storage_response: Ok(None),
        })));
        ext.execute_with(|| {
            assert!(matches!(
                db_events_at::<Test, Api>(H256::zero()),
                Err(DBEventError::Empty)
            ));
        });
    }

    #[test]
    fn decode_error_on_garbage_bytes() {
        let mut ext = new_test_ext();
        ext.register_extension(ClientExt(Arc::new(MockClientProvider {
            finalized_state: None,
            storage_response: Ok(Some(StorageData(alloc::vec![0xFF, 0xFF, 0xFF]))),
        })));
        ext.execute_with(|| {
            assert!(matches!(
                db_events_at::<Test, Api>(H256::zero()),
                Err(DBEventError::Decode { .. })
            ));
        });
    }

    #[test]
    fn decodes_and_filters_events() {
        let records = vec![
            event_record(pallet_tables::Event::<Test>::SchemaUpdated(
                None,
                UpdateTableList::default(),
            )),
            event_record(pallet_tables::Event::<Test>::TableDropped(
                None,
                TableType::CoreBlockchain,
                TableIdentifier::from_str_unchecked("TABLE1", "NAMESPACE"),
                Source::Ethereum,
            )),
            event_record(pallet_indexing::Event::<Test, Api>::QuorumReached {
                quorum: DataQuorum {
                    table: TableIdentifier::from_str_unchecked("TABLE2", "NAMESPACE"),
                    batch_id: BatchId::default(),
                    data_hash: H256::zero(),
                    block_number: Default::default(),
                    agreements: SubmitterList::default(),
                    dissents: SubmitterList::default(),
                    quorum_scope: QuorumScope::Public,
                },
                data: BoundedVec::try_from(vec![1u8, 2, 3]).unwrap(),
            }),
            event_record(pallet_balances::Event::<Test>::Endowed {
                account: AccountId32::new([0u8; 32]),
                free_balance: 0,
            }),
            event_record(pallet_tables::Event::<Test>::TableUuidUpdated {
                old_uuid: Default::default(),
                new_uuid: Default::default(),
                version: Default::default(),
                table: TableIdentifier::from_str_unchecked("TABLE3", "NAMESPACE"),
            }),
        ];
        let mut ext = new_test_ext();
        ext.register_extension(ClientExt(Arc::new(MockClientProvider {
            finalized_state: None,
            storage_response: Ok(Some(StorageData(records.encode()))),
        })));
        ext.execute_with(|| {
            let events: Vec<_> = db_events_at::<Test, Api>(H256::zero()).unwrap().collect();
            assert_eq!(
                events.len(),
                3
            );
            assert!(matches!(
                &events[0],
                DBEvent::SchemaUpdated(None, list) if list.is_empty()
            ));
            assert!(matches!(
                &events[1],
                DBEvent::TableDropped(None, TableType::CoreBlockchain, ident, Source::Ethereum)
                    if *ident == TableIdentifier::from_str_unchecked("TABLE1", "NAMESPACE")
            ));
            assert!(matches!(
                &events[2],
                DBEvent::QuorumReached { quorum, .. } if quorum.table == TableIdentifier::from_str_unchecked("TABLE2", "NAMESPACE")
            ));
        });
    }
}
