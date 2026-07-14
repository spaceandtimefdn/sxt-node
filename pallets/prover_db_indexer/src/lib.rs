//! # Prover-Db Indexer Pallet
//!
//! Forwards table lifecycle and data events to an external prover-db
//! indexer (HTTP) using protobuf-over-HTTP.
//!
//! ## Architecture
//!
//! **Producer** (extrinsic-time, via the [`EventCapture`] trait):
//!   `pallet-tables` and `pallet-indexing` call
//!   `T::EventCapture::capture_events(...)` at the same call site that
//!   deposits their schema/quorum events. The pallet writes the
//!   per-extrinsic payload to the offchain DB at
//!   `key_for_event(block, ext_idx)` and overwrites a per-block
//!   "high-water-mark" key at `key_for_high_water(block)` carrying the
//!   largest `ext_idx` that captured anything in this block. No on-chain
//!   state is needed — `extrinsic_index` is already available to the
//!   runtime, and absence of the high-water key means the block had no
//!   captures.
//!
//! **Consumer** (`offchain_worker`, fires at chain tip only):
//!   Asks the indexer server for its last checkpoint sequence number,
//!   walks blocks `cursor+1..=current` (capped at
//!   [`MAX_BLOCKS_PER_INVOCATION`]). For each block, reads the
//!   high-water-mark; if absent, the block had zero captures and is
//!   skipped. Otherwise probes `key_for_event(block, 0..=hwm)`,
//!   forwards each present payload via HTTP+protobuf, checkpoints on
//!   the server, and deletes consumed entries from the offchain DB.
//!
//! ## Why extrinsic-time capture, not `on_finalize`?
//!
//! Any synchronous runtime work has to declare its weight up front, and
//! `Hooks::on_finalize` declares its weight via the value `on_initialize`
//! returns at the *start* of the block. We have no way to know at
//! `on_initialize` how many table updates or quorum events the block
//! will produce, so we can't bound `on_finalize`'s cost in advance.
//! Capturing at the deposit-event call site instead lets the cost ride
//! on the calling extrinsic's already-benchmarked weight: every byte of
//! work is owned by an extrinsic that the chain has agreed to schedule.
//!
//! [`EventCapture`]: sxt_core::prover_db_indexer::EventCapture

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod http_client;
mod offchain_consumer;

/// Generated protobuf types for the indexer HTTP adapter wire format.
mod proto {
    include!(concat!(env!("OUT_DIR"), "/io.spaceandtime.indexer.rs"));
}

pub use pallet::*;
/// Re-export of the canonical offchain local-storage key (defined in `sxt-core`),
/// so the node service and any external tooling have a single import.
pub use sxt_core::prover_db_indexer::PROVER_DB_CONFIG_KEY;

/// Offchain DB key for the lock that serializes OCW consumer rounds.
const OCW_LOCK_KEY: &[u8] = b"prover_db_indexer/ocw_lock";

/// Typed errors from the OCW consumer round. Each variant carries the
/// underlying `http_client::Error` (when applicable) so the operator's
/// log line names both the failing operation and the wire-level reason.
///
/// `source` fields aren't named `source` because `url::ParseError`
/// doesn't implement `core::error::Error` in this crate's no_std build,
/// which would break Snafu's source-chain wiring. `http_client::Error`
/// could use the wired name, but we keep the convention uniform so
/// every variant looks the same.
#[derive(Debug, snafu::Snafu)]
pub enum ConsumerError {
    /// The configured indexer URL couldn't be parsed.
    #[snafu(display("indexer URL is not a valid URL: {error}"))]
    InvalidUrl {
        /// Underlying parse error from the `url` crate.
        error: url::ParseError,
    },
    /// The runtime's `BlockNumber` didn't fit in `u64` (defensive — in
    /// practice `BlockNumber` is `u32` so this is unreachable).
    #[snafu(display("block number does not fit in u64"))]
    BlockNumberOverflow,
    /// `create_table` HTTP call failed.
    #[snafu(display("create_table failed: {error}"))]
    CreateTable {
        /// Underlying error from the HTTP client.
        error: http_client::Error,
    },
    /// `drop_table` HTTP call failed.
    #[snafu(display("drop_table failed: {error}"))]
    DropTable {
        /// Underlying error from the HTTP client.
        error: http_client::Error,
    },
    /// `put_batches` HTTP call failed.
    #[snafu(display("put_batches failed: {error}"))]
    PutBatches {
        /// Underlying error from the HTTP client.
        error: http_client::Error,
    },
    /// `checkpoint` HTTP call failed.
    #[snafu(display("checkpoint failed: {error}"))]
    Checkpoint {
        /// Underlying error from the HTTP client.
        error: http_client::Error,
    },
}

#[polkadot_sdk::frame_support::pallet]
#[allow(
    missing_docs,
    clippy::missing_docs_in_private_items,
    clippy::manual_inspect,
    dead_code
)]
pub mod pallet {
    use alloc::vec::Vec;

    use codec::Encode;
    use polkadot_sdk::frame_support::pallet_prelude::*;
    use polkadot_sdk::frame_system::pallet_prelude::*;
    use polkadot_sdk::sp_runtime::offchain::storage::StorageValueRef;
    use polkadot_sdk::sp_runtime::offchain::storage_lock::{StorageLock, Time};
    use polkadot_sdk::sp_runtime::offchain::Duration;
    use sxt_core::prover_db_indexer::{
        key_for_event,
        key_for_high_water,
        table_matches_filters,
        BlockEvent,
        EventCapture,
        ProverDbConsumerConfig,
        TableIdentifierFilter,
    };

    use crate::ConsumerError;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config {
        /// The runtime's overarching event type.
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>;

        /// Maximum number of blocks the OCW will walk in a single
        /// invocation. Controls catch-up speed: with 6s block time and
        /// the default of 100, catch-up is ~100x realtime. Lower values
        /// keep the per-round cost bounded; higher values close a wider
        /// gap faster after downtime.
        #[pallet::constant]
        type MaxBlocksPerInvocation: Get<u64>;

        /// How long (in milliseconds) a held OCW lock stays valid before
        /// being treated as abandoned (e.g. node crashed mid-round). Must
        /// comfortably cover one full consumer round under normal
        /// conditions while still letting the chain recover quickly from
        /// a crashed validator.
        #[pallet::constant]
        type OcwLockDeadlineMs: Get<u64>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
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
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Fires at chain tip only (not during sync). Drains the offchain
        // DB queue, forwards to the HTTP server, deletes consumed entries.
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            if let Err(e) = Self::run_consumer(block_number) {
                polkadot_sdk::sp_tracing::error!(
                    target: "prover_db_indexer",
                    "offchain indexer error: {}",
                    e,
                );
            }
        }
    }

    impl<T: Config> EventCapture for Pallet<T> {
        fn capture_events(events: Vec<BlockEvent<'_>>) {
            if events.is_empty() {
                return;
            }

            // Capture every event unconditionally. Per-node filtering is
            // applied later by the OCW consumer when forwarding to the
            // indexer; the offchain queue mirrors the full block so every
            // validator carries the same data, regardless of which subset
            // each indexer cares about.

            // `block_number()` always fits in u64 in our runtime (BlockNumber
            // is u32); `extrinsic_index()` is always `Some` while we're inside
            // an extrinsic, which is the only path that reaches `capture_events`.
            // The `unwrap_or(0)` fallbacks are defensive, not load-bearing.
            let block: u64 = polkadot_sdk::frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(0);
            let ext_idx = polkadot_sdk::frame_system::Pallet::<T>::extrinsic_index().unwrap_or(0);

            // Per-extrinsic event payload.
            polkadot_sdk::sp_io::offchain_index::set(
                &key_for_event(block, ext_idx),
                &events.encode(),
            );

            // Per-block high-water-mark. Always overwrites; the OCW only
            // cares about the final value, which is the largest ext_idx
            // that fired this block.
            polkadot_sdk::sp_io::offchain_index::set(&key_for_high_water(block), &ext_idx.encode());
        }
    }

    impl<T: Config> Pallet<T> {
        // ═══════════════════════════════════════════════════════════════
        //  CONSUMER: drain offchain DB → HTTP → delete (called from OCW)
        // ═══════════════════════════════════════════════════════════════

        fn run_consumer(block_number: BlockNumberFor<T>) -> Result<(), ConsumerError> {
            // Serialize concurrent OCW invocations. Substrate spawns
            // `offchain_worker` for every imported block, and rounds can
            // overlap if one runs longer than block time. Without a lock,
            // both rounds would read the same server checkpoint and
            // submit duplicate `put_batches`/`create_table`/`drop_table`
            // calls; the second one's `checkpoint()` would then fail with
            // `failed_precondition`. Holding this lock for the full round
            // makes overlapping invocations no-ops.
            let mut lock = StorageLock::<Time>::with_deadline(
                crate::OCW_LOCK_KEY,
                Duration::from_millis(T::OcwLockDeadlineMs::get()),
            );
            let _guard = match lock.try_lock() {
                Ok(g) => g,
                Err(_deadline) => {
                    polkadot_sdk::sp_tracing::debug!(
                        target: "prover_db_indexer",
                        "another OCW round is already in progress; skipping",
                    );
                    return Ok(());
                }
            };

            // 1. Read the single offchain config. Absence ⇒ this node
            //    isn't an indexer; OCW is dormant. The node service
            //    writes the encoded `ProverDbConsumerConfig` at startup
            //    iff `--prover-db-url` was provided.
            let cfg_ref = StorageValueRef::persistent(crate::PROVER_DB_CONFIG_KEY);
            let Some(cfg) = cfg_ref.get::<ProverDbConsumerConfig>().ok().flatten() else {
                return Ok(());
            };
            let url =
                url::Url::parse(&cfg.url).map_err(|error| ConsumerError::InvalidUrl { error })?;
            let include_filters: Vec<TableIdentifierFilter> = cfg.include;

            polkadot_sdk::sp_tracing::debug!(
                target: "prover_db_indexer",
                "consumer round starting; indexer base URL = {}; {} include filters",
                url, include_filters.len(),
            );

            // 2. Current block number — passed in by `offchain_worker`.
            let current_block: u64 = block_number
                .try_into()
                .map_err(|_| ConsumerError::BlockNumberOverflow)?;

            // 3. Ask the server for its last checkpoint. The server is the
            //    sole source of truth — no local cursor. This costs one HTTP
            //    round-trip per OCW invocation but guarantees we never
            //    diverge from what the server has actually committed.
            //    A failure here is treated as transient: log and skip this
            //    round; the next OCW will retry.
            let cursor: u64 = match crate::http_client::get_last_checkpoint(&url) {
                Ok(Some(seq)) => seq,
                Ok(None) => 0,
                Err(e) => {
                    polkadot_sdk::sp_tracing::warn!(
                        target: "prover_db_indexer",
                        "get_last_checkpoint failed for {}: {}; skipping this round",
                        url, e,
                    );
                    return Ok(());
                }
            };

            // 4. Walk blocks from cursor+1 to tip, capped.
            let start = cursor + 1;
            let end = core::cmp::min(current_block, start + T::MaxBlocksPerInvocation::get() - 1);

            if start > current_block {
                return Ok(());
            }

            polkadot_sdk::sp_tracing::debug!(
                target: "prover_db_indexer",
                "processing blocks {}..={} (server_checkpoint={}, tip={})",
                start, end, cursor, current_block,
            );

            for block_num in start..=end {
                Self::forward_block(&url, block_num, &include_filters)?;

                // Checkpoint on the server (always, even for empty blocks).
                crate::http_client::checkpoint(&url, block_num)
                    .map_err(|error| ConsumerError::Checkpoint { error })?;
            }

            Ok(())
        }

        /// Forward a single block's events in extrinsic-index order, then
        /// clear the offchain entries we consumed. If the block had no
        /// captures (no high-water-mark key), this is a no-op.
        fn forward_block(
            url: &url::Url,
            block_num: u64,
            include_filters: &[TableIdentifierFilter],
        ) -> Result<(), ConsumerError> {
            let Some(hwm) = crate::offchain_consumer::read_high_water(block_num) else {
                return Ok(());
            };

            polkadot_sdk::sp_tracing::info!(
                target: "prover_db_indexer",
                "block {} — high-water-mark {}; probing for captured events",
                block_num,
                hwm,
            );

            for ext_idx in 0..=hwm {
                let Some(events) = crate::offchain_consumer::read_events(block_num, ext_idx) else {
                    continue;
                };
                Self::forward_events(url, block_num, &events, include_filters)?;
                crate::offchain_consumer::clear_events(block_num, ext_idx);
            }

            crate::offchain_consumer::clear_high_water(block_num);
            Ok(())
        }

        /// POST one extrinsic's captured events to the indexer in deposit
        /// order, skipping any whose table doesn't match this node's
        /// include set. The capture queue is unfiltered (every validator
        /// records the full block), so the filter applies only to the
        /// HTTP forwarding done by this node's OCW.
        fn forward_events(
            url: &url::Url,
            block_num: u64,
            events: &[BlockEvent<'_>],
            include_filters: &[TableIdentifierFilter],
        ) -> Result<(), ConsumerError> {
            let seq = block_num;

            for event in events {
                let table = match event {
                    BlockEvent::Create(entry) => entry.ident.as_ref(),
                    BlockEvent::Drop(ident) => ident.as_ref(),
                    BlockEvent::Insert(entry) => entry.table.as_ref(),
                };
                if !table_matches_filters(table, include_filters) {
                    continue;
                }

                match event {
                    BlockEvent::Drop(ident) => {
                        crate::http_client::drop_table(url, seq, ident)
                            .map_err(|error| ConsumerError::DropTable { error })?;
                    }
                    BlockEvent::Create(entry) => {
                        crate::http_client::create_table(
                            url,
                            seq,
                            &entry.ident,
                            entry.ddl.to_vec(),
                            commitment_sql::ROW_NUMBER_COLUMN_NAME.into(),
                        )
                        .map_err(|error| ConsumerError::CreateTable { error })?;
                    }
                    BlockEvent::Insert(entry) => {
                        crate::http_client::put_batches(
                            url,
                            seq,
                            alloc::vec![(entry.table.as_ref(), entry.data.to_vec())],
                        )
                        .map_err(|error| ConsumerError::PutBatches { error })?;
                    }
                }
            }

            Ok(())
        }
    }
}
