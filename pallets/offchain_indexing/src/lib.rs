//! # Offchain Indexing Pallet
//!
//! Offchain worker that forwards table lifecycle and data events to an
//! external HTTP indexer service using protobuf-over-HTTP (Option B).
//!
//! ## Overview
//!
//! On every block the OCW:
//! 1. Checks if this node is configured as an indexer (via local storage).
//! 2. Reads the current block's events from `frame_system::Pallet::events()`.
//! 3. Filters for `SchemaUpdated`, `TablesCreatedWithCommitments`,
//!    `TableDropped`, and `QuorumReached`.
//! 4. Translates them to protobuf and POSTs via `sp_io::offchain::http`.
//! 5. Checkpoints the block number on the server.
//! 6. Updates the local cursor.
//!
//! ## Important
//!
//! Events are only available during the block in which they were emitted.
//! `frame_system::Pallet::events()` is cleared at the start of each new block.
//! Therefore the OCW processes the **current block only** — no historical backfill.
//!
//! ## Configuration
//!
//! The indexer HTTP endpoint is stored in offchain local storage under a
//! well-known key. Set it via `offchain_localStorageSet` RPC.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "std")]
mod http_client;

#[cfg(feature = "std")]
mod translate;

#[cfg(feature = "std")]
mod proto {
    include!(concat!(env!("OUT_DIR"), "/io.spaceandtime.indexer.rs"));
}

pub use pallet::*;

/// Well-known offchain local-storage key for the indexer HTTP endpoint URL.
/// When this key is set (e.g. via `offchain_localStorageSet`), the OCW
/// activates. When absent, the OCW is a no-op.
pub const INDEXER_URL_KEY: &[u8] = b"offchain_indexing::indexer_url";

/// Well-known offchain local-storage key for the last successfully forwarded
/// block number. Persisted across OCW invocations via `StorageValueRef`.
pub const LAST_FORWARDED_BLOCK_KEY: &[u8] = b"offchain_indexing::last_forwarded_block";

/// Dedup key column name. Every table carries this column for replay safety.
#[cfg(feature = "std")]
const DEDUP_KEY_COLUMN: &str = "META_ROW_NUMBER";

#[frame_support::pallet]
pub mod pallet {
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec::Vec;

    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sxt_core::tables::{TableIdentifier, TableType};

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Tight coupling to the pallets whose events we forward. The OCW needs
    /// access to the concrete event types to match on them.
    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_tables::Config
    {
        /// The overarching runtime event type. Must support conversion from
        /// our event and from `pallet_tables::Event` (for event extraction).
        type RuntimeEvent: From<Event<Self>>
            + From<pallet_tables::Event<Self>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The offchain indexer successfully forwarded a block.
        BlockForwarded {
            /// Block number that was forwarded.
            block_number: u64,
        },
        /// The offchain indexer encountered an error forwarding a block.
        ForwardingError {
            /// Block number where the error occurred.
            block_number: u64,
        },
    }

    /// Intermediate representation of operations extracted from one block.
    #[cfg(feature = "std")]
    struct BlockOps {
        creates: Vec<(TableIdentifier, Vec<u8>)>, // (ident, ddl_bytes)
        drops: Vec<TableIdentifier>,
        data: BTreeMap<String, Vec<Vec<u8>>>,      // fq_name → postcard blobs
    }

    #[cfg(feature = "std")]
    impl Default for BlockOps {
        fn default() -> Self {
            Self {
                creates: Vec::new(),
                drops: Vec::new(),
                data: BTreeMap::new(),
            }
        }
    }

    #[cfg(feature = "std")]
    impl BlockOps {
        fn is_empty(&self) -> bool {
            self.creates.is_empty() && self.drops.is_empty() && self.data.is_empty()
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(_block_number: BlockNumberFor<T>) {
            #[cfg(feature = "std")]
            {
                if let Err(e) = Self::run_offchain_indexer() {
                    log::error!(
                        target: "offchain_indexing",
                        "offchain indexer error: {:?}",
                        e,
                    );
                }
            }
        }
    }

    #[cfg(feature = "std")]
    impl<T: Config> Pallet<T> {
        fn run_offchain_indexer() -> Result<(), &'static str> {
            use sp_runtime::offchain::storage::StorageValueRef;

            // 1. Check if this node is configured as an indexer.
            let url_ref = StorageValueRef::persistent(crate::INDEXER_URL_KEY);
            let url_bytes: Option<Vec<u8>> = url_ref.get::<Vec<u8>>().ok().flatten();

            let Some(url_bytes) = url_bytes else {
                return Ok(());
            };

            let url = String::from_utf8(url_bytes)
                .map_err(|_| "invalid UTF-8 in indexer URL")?;

            // 2. Get current block number.
            let current_block: u64 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .map_err(|_| "block number conversion failed")?;

            // 3. Check cursor — skip if we already forwarded this block.
            //    On first run (no local cursor), ask the server for its
            //    last checkpoint so we don't re-forward blocks it already has.
            let cursor_ref = StorageValueRef::persistent(crate::LAST_FORWARDED_BLOCK_KEY);
            let last_forwarded: Option<u64> = match cursor_ref.get::<u64>().ok().flatten() {
                Some(n) => Some(n),
                None => {
                    // First run: ask the server.
                    match crate::http_client::get_last_checkpoint(&url) {
                        Ok(Some(server_seq)) => {
                            log::info!(
                                target: "offchain_indexing",
                                "resuming from server checkpoint: {}",
                                server_seq,
                            );
                            // Persist so we don't ask again next block.
                            cursor_ref.set(&server_seq);
                            Some(server_seq)
                        }
                        Ok(None) => {
                            log::info!(
                                target: "offchain_indexing",
                                "no server checkpoint; starting fresh",
                            );
                            None
                        }
                        Err(e) => {
                            log::warn!(
                                target: "offchain_indexing",
                                "failed to get last checkpoint: {}; skipping this round",
                                e,
                            );
                            return Ok(());
                        }
                    }
                }
            };

            if let Some(last) = last_forwarded {
                if current_block <= last {
                    return Ok(());
                }
            }

            // 4. Extract events from the current block.
            let ops = Self::extract_events();

            if ops.is_empty() {
                log::debug!(
                    target: "offchain_indexing",
                    "block {} — no relevant events, checkpointing only",
                    current_block,
                );
            } else {
                log::info!(
                    target: "offchain_indexing",
                    "block {} — {} creates, {} drops, {} data tables",
                    current_block,
                    ops.creates.len(),
                    ops.drops.len(),
                    ops.data.len(),
                );
            }

            // 5. Forward events to the indexer.
            Self::forward_ops(&url, current_block, ops)?;

            // 6. Checkpoint on the server.
            crate::http_client::checkpoint(&url, current_block)
                .map_err(|_| "checkpoint failed")?;

            // 7. Update local cursor.
            cursor_ref.set(&current_block);

            log::info!(
                target: "offchain_indexing",
                "block {} forwarded and checkpointed",
                current_block,
            );

            Ok(())
        }

        /// Walk `frame_system::Pallet::<T>::events()` and extract the event
        /// variants we care about into a `BlockOps`.
        ///
        /// Events from the current block are available because the OCW fires
        /// after block execution but before the next block's `on_initialize`
        /// clears them.
        fn extract_events() -> BlockOps {
            let mut ops = BlockOps::default();

            for record in frame_system::Pallet::<T>::events() {
                // record.event is <T as frame_system::Config>::RuntimeEvent.
                // We encode it to SCALE bytes and dispatch by pallet index.
                Self::try_extract_table_event(&record.event, &mut ops);
                Self::try_extract_indexing_event(&record.event, &mut ops);
            }

            ops
        }

        /// Compute the SCALE-encoded pallet index byte for pallet_tables
        /// events. This avoids hardcoding the index, which differs between
        /// the production runtime and test mocks.
        fn tables_pallet_index() -> u8 {
            let dummy_ident = TableIdentifier {
                namespace: BoundedVec::try_from(Vec::new()).unwrap(),
                name: BoundedVec::try_from(Vec::new()).unwrap(),
            };
            let dummy = pallet_tables::Event::<T>::TableDropped(
                None,
                TableType::Community,
                dummy_ident,
            );
            // Convert via our pallet's RuntimeEvent (which has From<pallet_tables::Event>)
            // then use IsType to get the frame_system RuntimeEvent.
            let our_event: <T as Config>::RuntimeEvent = dummy.into();
            let runtime_event = <<T as Config>::RuntimeEvent as IsType<
                <T as frame_system::Config>::RuntimeEvent,
            >>::into_ref(&our_event);
            codec::Encode::encode(runtime_event)[0]
        }

        /// Attempt to match a table pallet event.
        fn try_extract_table_event(
            event: &<T as frame_system::Config>::RuntimeEvent,
            ops: &mut BlockOps,
        ) {
            use codec::Decode;

            let encoded = codec::Encode::encode(event);
            if encoded.is_empty() {
                return;
            }

            if encoded[0] != Self::tables_pallet_index() {
                return;
            }

            let inner = &encoded[1..];
            let Ok(table_event) =
                pallet_tables::Event::<T>::decode(&mut &inner[..])
            else {
                return;
            };

            match table_event {
                pallet_tables::Event::SchemaUpdated(_who, update_list) => {
                    for update in update_list.iter() {
                        ops.creates.push((
                            update.ident.clone(),
                            update.create_statement.to_vec(),
                        ));
                    }
                }
                pallet_tables::Event::TablesCreatedWithCommitments {
                    table_list,
                    ..
                } => {
                    for req in table_list.iter() {
                        ops.creates.push((
                            req.table_name.clone(),
                            req.ddl.to_vec(),
                        ));
                    }
                }
                pallet_tables::Event::TableDropped(_who, _table_type, ident) => {
                    ops.drops.push(ident);
                }
                _ => {}
            }
        }

        /// Attempt to match an indexing pallet event (QuorumReached).
        /// Since pallet_indexing uses a generic `I` parameter, we use raw
        /// SCALE decoding of the inner fields rather than tight coupling.
        fn try_extract_indexing_event(
            event: &<T as frame_system::Config>::RuntimeEvent,
            ops: &mut BlockOps,
        ) {
            use codec::Decode;
            use sxt_core::indexing::DataQuorum;

            let encoded = codec::Encode::encode(event);
            if encoded.len() < 2 {
                return;
            }

            // Skip events we already handled (tables pallet).
            if encoded[0] == Self::tables_pallet_index() {
                return;
            }

            // QuorumReached is variant index 1 within the indexing pallet.
            // We try to decode the full QuorumReached payload from byte 2
            // onwards (byte 0 = pallet index, byte 1 = event variant index).
            if encoded[1] != 1 {
                return;
            }

            let mut input = &encoded[2..];

            let Ok(quorum) = DataQuorum::<
                <T as frame_system::Config>::AccountId,
                <T as frame_system::Config>::Hash,
            >::decode(&mut input) else {
                // Not a QuorumReached or wrong pallet — skip silently.
                return;
            };

            let Ok(data) = <sp_runtime::BoundedVec<u8, frame_support::traits::ConstU32<{ sxt_core::indexing::DATA_MAX_LEN }>>>::decode(&mut input) else {
                log::warn!(target: "offchain_indexing", "failed to decode QuorumReached data");
                return;
            };

            let fq = crate::translate::fq_name(&quorum.table);
            ops.data.entry(fq).or_default().push(data.to_vec());
        }

        /// Send all extracted operations to the indexer HTTP service.
        fn forward_ops(
            url: &str,
            block_number: u64,
            ops: BlockOps,
        ) -> Result<(), &'static str> {
            let seq = block_number;

            // Drops first.
            for ident in ops.drops {
                let name = crate::translate::fq_name(&ident);
                crate::http_client::drop_table(url, seq, name)
                    .map_err(|_| "drop_table failed")?;
            }

            // Creates.
            for (ident, ddl) in ops.creates {
                let name = crate::translate::fq_name(&ident);
                let schema = crate::translate::ddl_to_arrow_ipc_schema(&ddl)
                    .map_err(|_| "DDL translation failed")?;
                crate::http_client::create_table(
                    url,
                    seq,
                    name,
                    schema,
                    crate::DEDUP_KEY_COLUMN.into(),
                )
                .map_err(|_| "create_table failed")?;
            }

            // Data batches — one PutBatches call with all tables.
            if !ops.data.is_empty() {
                let mut batches = Vec::new();
                for (table_name, blobs) in ops.data {
                    let ipc = crate::translate::on_chain_tables_to_arrow_ipc(&blobs)
                        .map_err(|_| "Arrow IPC translation failed")?;
                    batches.push(crate::proto::TableBatch {
                        table_name,
                        record_batch: ipc,
                    });
                }
                crate::http_client::put_batches(url, seq, batches)
                    .map_err(|_| "put_batches failed")?;
            }

            Ok(())
        }
    }
}
