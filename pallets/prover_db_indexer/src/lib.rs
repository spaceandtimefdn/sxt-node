//! # Prover-Db Indexer Pallet
//!
//! Forwards table lifecycle and data events to an external prover-db
//! indexer (HTTP) using protobuf-over-HTTP.
//!
//! ## Architecture
//!
//! **Producer** (`on_finalize`, runs during block execution including sync):
//!   Reads `polkadot_sdk::frame_system::Events`, extracts relevant variants, writes a
//!   SCALE-encoded `BlockIndex` to the offchain DB keyed by block number.
//!
//! **Consumer** (`offchain_worker`, fires at chain tip only):
//!   Walks the offchain DB from `cursor+1` to `current_block`, forwards each
//!   entry via HTTP+protobuf, checkpoints on the server, deletes consumed
//!   entries, advances cursor.
//!
//! ## Why two hooks?
//!
//! `on_finalize` can read events (they exist during block execution) but
//! cannot do HTTP. `offchain_worker` can do HTTP but cannot read past events
//! (cleared at next block). The offchain DB bridges them.
//!
//! ## No new host functions
//!
//! Raw DDL bytes (for schemas) and postcard-encoded `OnChainTable` bytes
//! (for row data — what `pallet_indexing::QuorumReached.data` carries,
//! after the chain has converted the indexer's Arrow IPC submission to
//! an augmented `OnChainTable` and postcard-serialized it in
//! `finalize_quorum`) are shipped as-is. The HTTP server decodes both.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod http_client;
mod offchain_index;

pub mod native_pallet;

/// Generated protobuf types for the indexer HTTP adapter wire format.
#[allow(missing_docs, clippy::missing_docs_in_private_items)]
mod proto {
    include!(concat!(env!("OUT_DIR"), "/io.spaceandtime.indexer.rs"));
}

pub use pallet::*;

/// Offchain local-storage key for the prover-db indexer HTTP endpoint URL.
pub const PROVER_DB_URL_KEY: &[u8] = b"prover_db_indexer::prover_db_url";

/// Dedup key column name sent with every `CreateTable` request.
const DEDUP_KEY_COLUMN: &str = "META_ROW_NUMBER";

/// Maximum blocks to process per OCW invocation. Controls catch-up speed.
/// At 100 blocks/invocation and 6s block time, catch-up is ~100x realtime.
const MAX_BLOCKS_PER_INVOCATION: u64 = 100;

#[polkadot_sdk::frame_support::pallet]
#[allow(missing_docs, clippy::missing_docs_in_private_items, dead_code)]
pub mod pallet {
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;

    use polkadot_sdk::frame_support::pallet_prelude::*;
    use polkadot_sdk::frame_system::pallet_prelude::*;
    use sxt_core::tables::TableIdentifier;

    use crate::offchain_index::{BlockEvent, BlockIndex, CreateEntry, DataEntry};

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::config]
    pub trait Config<I: 'static = ()>:
        polkadot_sdk::frame_system::Config + pallet_tables::Config + pallet_indexing::Config<I>
    {
        /// The runtime's overarching event type. Bounds let us downcast
        /// it back to the per-pallet typed events without re-encoding,
        /// using `TryInto` impls that `construct_runtime!` generates.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>
            + TryInto<pallet_tables::Event<Self>>
            + TryInto<pallet_indexing::Event<Self, I>>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// The offchain indexer successfully forwarded a block.
        BlockForwarded {
            /// Block number that was successfully forwarded.
            block_number: u64,
        },
        /// The offchain indexer encountered an error.
        ForwardingError {
            /// Block number that the forwarder failed on.
            block_number: u64,
        },
    }

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
        // ─── PRODUCER ───────────────────────────────────────────────────
        // Runs during block execution (including sync). Captures events
        // and persists them to the offchain DB for the OCW to consume.
        fn on_finalize(n: BlockNumberFor<T>) {
            let block_number: u64 = n.try_into().unwrap_or(0);
            let index = Self::extract_block_index();

            if index.is_empty() {
                return;
            }

            log::debug!(
                target: "prover_db_indexer",
                "on_finalize({}): writing {} events to offchain DB",
                block_number,
                index.events.len(),
            );

            crate::offchain_index::write(block_number, &index);
        }

        // ─── CONSUMER ───────────────────────────────────────────────────
        // Fires at chain tip only (not during sync). Drains the offchain
        // DB queue, forwards to the HTTP server, deletes consumed entries.
        fn offchain_worker(_block_number: BlockNumberFor<T>) {
            if let Err(e) = Self::run_consumer() {
                log::error!(
                    target: "prover_db_indexer",
                    "offchain indexer error: {:?}",
                    e,
                );
            }
        }
    }

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        // ═══════════════════════════════════════════════════════════════
        //  PRODUCER: extract events → BlockIndex (called from on_finalize)
        // ═══════════════════════════════════════════════════════════════

        fn extract_block_index() -> BlockIndex {
            let mut index = BlockIndex::default();

            for record in polkadot_sdk::frame_system::Pallet::<T>::read_events_no_consensus() {
                // `record.event` is the outer `RuntimeEvent`. We try
                // converting it back to each typed pallet event via
                // `TryInto` impls generated by `construct_runtime!`.
                // Cloning is cheap — events are small SCALE enums — and
                // saves us writing two extraction passes back-to-back.
                Self::try_extract_table_event(record.event.clone(), &mut index);
                Self::try_extract_indexing_event(record.event, &mut index);
            }

            index
        }

        fn try_extract_table_event(
            event: <T as polkadot_sdk::frame_system::Config>::RuntimeEvent,
            index: &mut BlockIndex,
        ) {
            // `event` is the frame-system RuntimeEvent. Bounce through our
            // Config's RuntimeEvent (same type at runtime, bridged by `IsType`)
            // so the `TryInto<pallet_tables::Event>` bound applies.
            let our_event = <<T as Config<I>>::RuntimeEvent as From<_>>::from(event);
            let Ok(table_event): Result<pallet_tables::Event<T>, _> = our_event.try_into() else {
                return;
            };
            match table_event {
                pallet_tables::Event::SchemaUpdated(_who, update_list) => {
                    for update in update_list.iter() {
                        index.events.push(BlockEvent::Create(CreateEntry {
                            ident: update.ident.clone(),
                            ddl: update.create_statement.to_vec(),
                        }));
                    }
                }
                pallet_tables::Event::TablesCreatedWithCommitments { table_list, .. } => {
                    for req in table_list.iter() {
                        index.events.push(BlockEvent::Create(CreateEntry {
                            ident: req.table_name.clone(),
                            ddl: req.ddl.to_vec(),
                        }));
                    }
                }
                pallet_tables::Event::TableDropped(_who, _table_type, ident, _source) => {
                    index.events.push(BlockEvent::Drop(ident));
                }
                _ => {}
            }
        }

        fn try_extract_indexing_event(
            event: <T as polkadot_sdk::frame_system::Config>::RuntimeEvent,
            index: &mut BlockIndex,
        ) {
            let our_event = <<T as Config<I>>::RuntimeEvent as From<_>>::from(event);
            let Ok(indexing_event): Result<pallet_indexing::Event<T, I>, _> = our_event.try_into()
            else {
                return;
            };
            if let pallet_indexing::Event::QuorumReached { quorum, data } = indexing_event {
                index.events.push(BlockEvent::Data(DataEntry {
                    table: quorum.table,
                    data: data.to_vec(),
                }));
            }
        }

        // ═══════════════════════════════════════════════════════════════
        //  CONSUMER: drain offchain DB → HTTP → delete (called from OCW)
        // ═══════════════════════════════════════════════════════════════

        fn run_consumer() -> Result<(), &'static str> {
            use polkadot_sdk::sp_runtime::offchain::storage::StorageValueRef;

            // 1. Check if this node is configured as an indexer.
            let url_ref = StorageValueRef::persistent(crate::PROVER_DB_URL_KEY);
            let url_bytes: Option<Vec<u8>> = url_ref.get::<Vec<u8>>().ok().flatten();

            let Some(url_bytes) = url_bytes else {
                return Ok(());
            };

            let url =
                core::str::from_utf8(&url_bytes).map_err(|_| "invalid UTF-8 in indexer URL")?;

            // 2. Current block number.
            let current_block: u64 = polkadot_sdk::frame_system::Pallet::<T>::block_number()
                .try_into()
                .map_err(|_| "block number conversion failed")?;

            // 3. Ask the server for its last checkpoint. The server is the
            //    sole source of truth — no local cursor. This costs one HTTP
            //    round-trip per OCW invocation but guarantees we never
            //    diverge from what the server has actually committed.
            let cursor: u64 = match crate::http_client::get_last_checkpoint(url) {
                Ok(Some(seq)) => seq,
                Ok(None) => 0,
                Err(e) => {
                    log::warn!(
                        target: "prover_db_indexer",
                        "get_last_checkpoint failed: {}; skipping this round",
                        e,
                    );
                    return Ok(());
                }
            };

            // 4. Walk blocks from cursor+1 to tip, capped.
            let start = cursor + 1;
            let end = core::cmp::min(current_block, start + crate::MAX_BLOCKS_PER_INVOCATION - 1);

            if start > current_block {
                return Ok(());
            }

            log::debug!(
                target: "prover_db_indexer",
                "processing blocks {}..={} (server_checkpoint={}, tip={})",
                start, end, cursor, current_block,
            );

            for block_num in start..=end {
                // Read from offchain DB.
                let entry = crate::offchain_index::read(block_num);

                match &entry {
                    Some(idx) if !idx.is_empty() => {
                        log::info!(
                            target: "prover_db_indexer",
                            "block {} — {} events to forward",
                            block_num,
                            idx.events.len(),
                        );
                        Self::forward_block(url, block_num, idx)?;
                    }
                    _ => {}
                }

                // Checkpoint on the server (always, even for empty blocks).
                crate::http_client::checkpoint(url, block_num).map_err(|_| "checkpoint failed")?;

                // Delete consumed entry from offchain DB.
                if entry.is_some() {
                    crate::offchain_index::clear(block_num);
                }
            }

            Ok(())
        }

        /// Forward a single block's events in deposit order.
        fn forward_block(
            url: &str,
            block_num: u64,
            index: &BlockIndex,
        ) -> Result<(), &'static str> {
            use crate::offchain_index::BlockEvent;

            let seq = block_num;

            for event in &index.events {
                match event {
                    BlockEvent::Drop(ident) => {
                        let name = Self::fq_name(ident);
                        crate::http_client::drop_table(url, seq, name)
                            .map_err(|_| "drop_table failed")?;
                    }
                    BlockEvent::Create(entry) => {
                        let name = Self::fq_name(&entry.ident);
                        crate::http_client::create_table(
                            url,
                            seq,
                            name,
                            entry.ddl.clone(),
                            crate::DEDUP_KEY_COLUMN.into(),
                        )
                        .map_err(|_| "create_table failed")?;
                    }
                    BlockEvent::Data(entry) => {
                        let name = Self::fq_name(&entry.table);
                        crate::http_client::put_batches(
                            url,
                            seq,
                            alloc::vec![crate::proto::TableBatch {
                                table_name: name,
                                record_batch: entry.data.clone(),
                            }],
                        )
                        .map_err(|_| "put_batches failed")?;
                    }
                }
            }

            Ok(())
        }

        fn fq_name(id: &TableIdentifier) -> String {
            let ns = core::str::from_utf8(&id.namespace).unwrap_or("?");
            let name = core::str::from_utf8(&id.name).unwrap_or("?");
            format!("{}.{}", ns, name)
        }
    }
}
