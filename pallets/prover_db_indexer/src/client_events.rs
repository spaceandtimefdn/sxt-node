//! PoC: read events straight from the node's client (via the `ClientExt` runtime interface
//! from `native::client`) instead of replaying the `prover_db_indexer/event` offchain-DB
//! entries written by `EventCapture`.

use alloc::string::String;
use alloc::vec::Vec;

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
pub enum CapturedEventsError {
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
pub enum CapturedEvent<T: pallet_tables::Config + pallet_indexing::Config> {
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
}

impl<T> TryFrom<EventRecord<T>> for CapturedEvent<T>
where
    T: pallet_tables::Config + pallet_indexing::Config,
    RuntimeEvent<T>: TryInto<pallet_tables::Event<T>> + TryInto<pallet_indexing::Event<T>>,
{
    type Error = ();

    fn try_from(record: EventRecord<T>) -> Result<Self, Self::Error> {
        if let Ok(event) = record.event.clone().try_into() {
            match event {
                pallet_tables::Event::SchemaUpdated(owner, tables) => {
                    Ok(CapturedEvent::SchemaUpdated(owner, tables))
                }
                pallet_tables::Event::TableDropped(owner, table_type, table, source) => Ok(
                    CapturedEvent::TableDropped(owner, table_type, table, source),
                ),
                _ => Err(()),
            }
        } else if let Ok(pallet_indexing::Event::QuorumReached { quorum, data }) =
            record.event.try_into()
        {
            Ok(CapturedEvent::QuorumReached { quorum, data })
        } else {
            Err(())
        }
    }
}

/// Queries the node's client for captured events at the given block hash.
pub fn captured_events_at<T>(block_hash: H256) -> Result<Vec<CapturedEvent<T>>, CapturedEventsError>
where
    T: pallet_tables::Config + pallet_indexing::Config,
    EventRecord<T>: TryInto<CapturedEvent<T>>,
{
    let events_key = Events::<T>::hashed_key().to_vec();

    let raw = match native::client::client::storage_at(block_hash, events_key) {
        None => return Err(CapturedEventsError::NoClient),
        Some(Err(message)) => return Err(CapturedEventsError::Lookup { message }),
        Some(Ok(None)) => return Err(CapturedEventsError::Empty),
        Some(Ok(Some(raw))) => raw,
    };

    Ok(Vec::<EventRecord<T>>::decode(&mut raw.0.as_slice())
        .map_err(|error| CapturedEventsError::Decode { error })?
        .into_iter()
        .filter_map(|record| record.try_into().ok())
        .collect())
}
