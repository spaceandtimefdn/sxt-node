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
//! This PR ships the producer half. The consumer (HTTP forwarding) is
//! added in a follow-up PR; until that lands, `offchain_worker` is a
//! no-op stub and any `BlockIndex` entries the producer writes simply
//! sit in the offchain DB.
//!
//! ## Why two hooks?
//!
//! `on_finalize` can read events (they exist during block execution) but
//! cannot do HTTP. `offchain_worker` can do HTTP but cannot read past events
//! (cleared at next block). The offchain DB bridges them.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod offchain_index;

pub mod native_pallet;

pub use pallet::*;
/// Re-export of the canonical offchain local-storage key (defined in `sxt-core`),
/// kept here so existing call sites that reach for `pallet_prover_db_indexer::PROVER_DB_URL_KEY`
/// keep working.
pub use sxt_core::PROVER_DB_URL_KEY;

#[polkadot_sdk::frame_support::pallet]
#[allow(missing_docs, clippy::missing_docs_in_private_items, dead_code)]
pub mod pallet {
    use alloc::vec::Vec;

    use polkadot_sdk::frame_support::pallet_prelude::*;
    use polkadot_sdk::frame_system::pallet_prelude::*;

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
        // Stubbed in this PR; HTTP forwarding logic lands in the follow-up.
        fn offchain_worker(_block_number: BlockNumberFor<T>) {}
    }

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        // ═══════════════════════════════════════════════════════════════
        //  PRODUCER: extract events → BlockIndex (called from on_finalize)
        // ═══════════════════════════════════════════════════════════════

        fn extract_block_index() -> BlockIndex {
            // Each frame-system event may produce zero or more `BlockEvent`s
            // (a `pallet_tables::SchemaUpdated` carries N updates, an
            // indexing quorum yields one, anything else yields nothing).
            // The extractors are pure: they take an event and return what
            // would be appended, leaving accumulation to the caller.
            let events = polkadot_sdk::frame_system::Pallet::<T>::read_events_no_consensus()
                .flat_map(|record| {
                    let from_tables = Self::try_extract_table_event(record.event.clone());
                    let from_indexing = Self::try_extract_indexing_event(record.event);
                    from_tables.into_iter().chain(from_indexing)
                })
                .collect();
            BlockIndex { events }
        }

        fn try_extract_table_event(
            event: <T as polkadot_sdk::frame_system::Config>::RuntimeEvent,
        ) -> Vec<BlockEvent> {
            // `event` is the frame-system RuntimeEvent. Bounce through our
            // Config's RuntimeEvent (same type at runtime, bridged by `IsType`)
            // so the `TryInto<pallet_tables::Event>` bound applies.
            let our_event = <<T as Config<I>>::RuntimeEvent as From<_>>::from(event);
            let Ok(table_event): Result<pallet_tables::Event<T>, _> = our_event.try_into() else {
                return Vec::new();
            };
            match table_event {
                pallet_tables::Event::SchemaUpdated(_who, update_list) => update_list
                    .iter()
                    .map(|update| {
                        BlockEvent::Create(CreateEntry {
                            ident: update.ident.clone(),
                            ddl: update.create_statement.to_vec(),
                        })
                    })
                    .collect(),
                pallet_tables::Event::TablesCreatedWithCommitments { table_list, .. } => table_list
                    .iter()
                    .map(|req| {
                        BlockEvent::Create(CreateEntry {
                            ident: req.table_name.clone(),
                            ddl: req.ddl.to_vec(),
                        })
                    })
                    .collect(),
                pallet_tables::Event::TableDropped(_who, _table_type, ident, _source) => {
                    alloc::vec![BlockEvent::Drop(ident)]
                }
                _ => Vec::new(),
            }
        }

        fn try_extract_indexing_event(
            event: <T as polkadot_sdk::frame_system::Config>::RuntimeEvent,
        ) -> Option<BlockEvent> {
            let our_event = <<T as Config<I>>::RuntimeEvent as From<_>>::from(event);
            let indexing_event: pallet_indexing::Event<T, I> = our_event.try_into().ok()?;
            let pallet_indexing::Event::QuorumReached { quorum, data } = indexing_event else {
                return None;
            };
            Some(BlockEvent::Data(DataEntry {
                table: quorum.table,
                data: data.to_vec(),
            }))
        }
    }
}
