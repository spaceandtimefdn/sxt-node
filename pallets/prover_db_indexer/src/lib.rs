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
//!   skipped. Otherwise probes `key_for_event(block, 0..=high_water_mark)`,
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

#[expect(dead_code, reason = "Usage for this function is not yet implemented")]
mod db_events;
mod http_client;
mod offchain_consumer;

/// Generated protobuf types for the indexer HTTP adapter wire format.
mod proto {
    include!(concat!(env!("OUT_DIR"), "/io.spaceandtime.indexer.rs"));
}

pub use pallet::*;

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
    /// Building the consumer configuration from node-supplied config failed.
    #[snafu(transparent)]
    Config {
        /// Underlying config-parsing error.
        source: sxt_core::prover_db_indexer::ProverDbConsumerConfigError,
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
    /// `get_last_checkpoint` HTTP call failed.
    #[snafu(display("get_last_checkpoint failed: {error}"))]
    GetLastCheckpoint {
        /// Underlying error from the HTTP client.
        error: http_client::Error,
    },
    /// Another OCW round is already in progress (lock held by another thread).
    #[snafu(display("another OCW round is already in progress"))]
    ConsumerInProgress,
    /// The [`ClientExt`] externalities extension is not registered, so the
    /// OCW can't read the node's client.
    #[snafu(display("no registered client"))]
    NoRegisteredClient,
    /// The node's client has no finalized state.
    #[snafu(display("missing finalized state"))]
    MissingFinalizedState,
    /// The finalized block hash from the node's client doesn't match the
    /// hash of the finalized block number. This indicates a serious
    /// inconsistency in the node's client state.
    #[snafu(display("finalized block hash mismatch"))]
    FinalizedBlockHashMismatch,
}

#[polkadot_sdk::frame_support::pallet]
#[allow(clippy::manual_inspect)]
pub mod pallet {
    use alloc::vec::Vec;

    use codec::Encode;
    use polkadot_sdk::frame_support::pallet_prelude::*;
    use polkadot_sdk::frame_system;
    use polkadot_sdk::frame_system::pallet_prelude::*;
    use polkadot_sdk::sp_core::H256;
    use polkadot_sdk::sp_runtime::offchain::storage_lock::{StorageLock, Time};
    use polkadot_sdk::sp_runtime::offchain::Duration;
    use polkadot_sdk::sp_runtime::traits::CheckedConversion;
    use polkadot_sdk::sp_runtime::SaturatedConversion;
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
    pub trait Config: polkadot_sdk::frame_system::Config<Hash = H256> {}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Fires at chain tip only (not during sync). Drains the offchain
        // DB queue, forwards to the HTTP server, deletes consumed entries.
        fn offchain_worker(_block_number: BlockNumberFor<T>) {
            if let Err(e) = Self::run_consumer() {
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
            // Capture every event unconditionally. Per-node filtering is
            // applied later by the OCW consumer when forwarding to the
            // indexer; the offchain queue mirrors the full block so every
            // validator carries the same data, regardless of which subset
            // each indexer cares about.

            // `block_number()` always fits in u64 in our runtime (BlockNumber
            // is u32); `extrinsic_index()` is always `Some` while we're inside
            // an extrinsic, which is the only path that reaches `capture_events`.
            let block: u64 =
                polkadot_sdk::frame_system::Pallet::<T>::block_number().saturated_into();
            let ext_idx =
                polkadot_sdk::frame_system::Pallet::<T>::extrinsic_index().unwrap_or(u32::MAX);

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

        fn run_consumer() -> Result<(), ConsumerError> {
            let config = ProverDbConsumerConfig::try_from_map(native::config::config::get)?;
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
                Duration::from_millis(config.ocw_lock_deadline_ms),
            );
            let _guard = lock
                .try_lock()
                .map_err(|_| ConsumerError::ConsumerInProgress)?;

            polkadot_sdk::sp_tracing::debug!(
                target: "prover_db_indexer",
                "consumer round starting; indexer base URL = {}; {} include filters",
                config.url, config.include.len(),
            );

            let (finalized_block_hash, finalized_block_num) =
                native::client::client::finalized_state()
                    .ok_or(ConsumerError::NoRegisteredClient)?
                    .ok_or(ConsumerError::MissingFinalizedState)?;
            let finalized_bn: BlockNumberFor<T> = finalized_block_num
                .checked_into()
                .ok_or(ConsumerError::BlockNumberOverflow)?;
            let expected_hash = frame_system::Pallet::<T>::block_hash(finalized_bn);
            if finalized_block_hash != expected_hash {
                return Err(ConsumerError::FinalizedBlockHashMismatch);
            }

            let cursor: u64 = crate::http_client::get_last_checkpoint(&config.url)
                .map_err(|error| ConsumerError::GetLastCheckpoint { error })?
                .unwrap_or(0);

            polkadot_sdk::sp_tracing::debug!(
                target: "prover_db_indexer",
                "processing blocks (server_checkpoint={}, tip={})",
                cursor, finalized_block_num,
            );

            for block_num in (cursor..=u64::from(finalized_block_num))
                .skip(1)
                .take(config.max_blocks_per_invocation)
            {
                Self::forward_block(&config.url, block_num, &config.include)?;

                // Checkpoint on the server (always, even for empty blocks).
                crate::http_client::checkpoint(&config.url, block_num)
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
            let Some(high_water_mark) = crate::offchain_consumer::read_high_water(block_num) else {
                return Ok(());
            };

            polkadot_sdk::sp_tracing::info!(
                target: "prover_db_indexer",
                "block {} — high-water-mark {}; probing for captured events",
                block_num,
                high_water_mark,
            );

            for ext_idx in 0..=high_water_mark {
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
            let filtered_events = events
                .iter()
                .filter(|event| table_matches_filters(event.table(), include_filters));
            for event in filtered_events {
                match event {
                    BlockEvent::Drop(ident) => {
                        crate::http_client::drop_table(url, block_num, ident)
                            .map_err(|error| ConsumerError::DropTable { error })?;
                    }
                    BlockEvent::Create(entry) => {
                        crate::http_client::create_table(
                            url,
                            block_num,
                            &entry.ident,
                            entry.ddl.to_vec(),
                            commitment_sql::ROW_NUMBER_COLUMN_NAME.into(),
                        )
                        .map_err(|error| ConsumerError::CreateTable { error })?;
                    }
                    BlockEvent::Insert(entry) => {
                        crate::http_client::put_batches(
                            url,
                            block_num,
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
