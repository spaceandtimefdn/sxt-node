#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod db_events;
mod http_client;

/// Generated protobuf types for the indexer HTTP adapter wire format.
mod proto {
    include!(concat!(env!("OUT_DIR"), "/io.spaceandtime.indexer.rs"));
}

pub use pallet::*;

/// Typed errors from the OCW consumer round. These are not `DispatchError`s
/// since these are handled silently in the OCW and logged, not propagated to the runtime.
#[derive(Debug, snafu::Snafu)]
#[snafu(module)]
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
    #[snafu(display("create_table failed: {source}"))]
    CreateTable {
        /// Underlying error from the HTTP client.
        source: http_client::Error,
    },
    /// `drop_table` HTTP call failed.
    #[snafu(display("drop_table failed: {source}"))]
    DropTable {
        /// Underlying error from the HTTP client.
        source: http_client::Error,
    },
    /// `put_batches` HTTP call failed.
    #[snafu(display("put_batches failed: {source}"))]
    PutBatches {
        /// Underlying error from the HTTP client.
        source: http_client::Error,
    },
    /// `checkpoint` HTTP call failed.
    #[snafu(display("checkpoint failed: {source}"))]
    Checkpoint {
        /// Underlying error from the HTTP client.
        source: http_client::Error,
    },
    /// `get_last_checkpoint` HTTP call failed.
    #[snafu(display("get_last_checkpoint failed: {source}"))]
    GetLastCheckpoint {
        /// Underlying error from the HTTP client.
        source: http_client::Error,
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
    /// Querying the node's client for a block's captured events failed.
    #[snafu(transparent)]
    DBEvents {
        /// Underlying error from [`crate::db_events::db_events_at`].
        source: crate::db_events::DBEventError,
    },
}

#[polkadot_sdk::frame_support::pallet]
#[allow(clippy::manual_inspect)]
pub mod pallet {
    use polkadot_sdk::frame_support::pallet_prelude::*;
    use polkadot_sdk::frame_system;
    use polkadot_sdk::frame_system::pallet_prelude::*;
    use polkadot_sdk::sp_core::H256;
    use polkadot_sdk::sp_runtime::offchain::storage_lock::{StorageLock, Time};
    use polkadot_sdk::sp_runtime::offchain::Duration;
    use polkadot_sdk::sp_runtime::traits::CheckedConversion;
    use snafu::{OptionExt, ResultExt};
    use sxt_core::prover_db_indexer::{
        table_matches_filters,
        ProverDbConsumerConfig,
        OCW_LOCK_KEY,
    };

    use crate::consumer_error::*;
    use crate::db_events::{db_events_at, DBEvent, EventRecord};
    use crate::ConsumerError;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        polkadot_sdk::frame_system::Config<Hash = H256>
        + pallet_tables::Config
        + pallet_indexing::Config<native_api::Api>
    {
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
    where
        EventRecord<T>: TryInto<DBEvent<T>>,
    {
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

    impl<T: Config> Pallet<T>
    where
        EventRecord<T>: TryInto<DBEvent<T>>,
    {
        fn run_consumer() -> Result<(), ConsumerError> {
            let config = ProverDbConsumerConfig::try_from_map(native::config::config::get)?;
            // Serialize concurrent OCW invocations. Substrate spawns
            // `offchain_worker` for every imported block, and rounds can
            // overlap if one runs longer than block time.
            let mut lock = StorageLock::<Time>::with_deadline(
                OCW_LOCK_KEY,
                Duration::from_millis(config.ocw_lock_deadline_ms),
            );
            let _guard = lock.try_lock().ok().context(ConsumerInProgressSnafu)?;

            polkadot_sdk::sp_tracing::debug!(
                target: "prover_db_indexer",
                "consumer round starting; indexer base URL = {}; {} include filters",
                config.url, config.include.len(),
            );

            let (finalized_block_hash, finalized_block_num) =
                native::client::client::finalized_state()
                    .context(NoRegisteredClientSnafu)?
                    .context(MissingFinalizedStateSnafu)?;
            let finalized_bn: BlockNumberFor<T> = finalized_block_num
                .checked_into()
                .context(BlockNumberOverflowSnafu)?;
            let expected_hash = frame_system::Pallet::<T>::block_hash(finalized_bn);
            snafu::ensure!(
                finalized_block_hash == expected_hash,
                FinalizedBlockHashMismatchSnafu
            );

            let cursor: u64 = crate::http_client::get_last_checkpoint(&config.url)
                .context(GetLastCheckpointSnafu)?
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
                Self::forward_block(block_num, &config)?;

                // Checkpoint on the server (always, even for empty blocks).
                crate::http_client::checkpoint(&config.url, block_num).context(CheckpointSnafu)?;
            }

            Ok(())
        }

        /// Forward a single block's events.
        fn forward_block(
            block_num: u64,
            config: &ProverDbConsumerConfig,
        ) -> Result<(), ConsumerError> {
            let bn: BlockNumberFor<T> =
                block_num.checked_into().context(BlockNumberOverflowSnafu)?;
            for event in db_events_at::<T>(frame_system::Pallet::<T>::block_hash(bn))? {
                match event {
                    DBEvent::TableDropped(_, _, table, _) => {
                        if table_matches_filters(&table, &config.include) {
                            crate::http_client::drop_table(&config.url, block_num, &table)
                                .context(DropTableSnafu)?;
                        }
                    }
                    DBEvent::SchemaUpdated(_, updates) => {
                        updates
                            .into_iter()
                            .filter(|update| table_matches_filters(&update.ident, &config.include))
                            .try_for_each(|update| {
                                crate::http_client::create_table(
                                    &config.url,
                                    block_num,
                                    &update.ident,
                                    update.create_statement.into_inner(),
                                    commitment_sql::ROW_NUMBER_COLUMN_NAME.into(),
                                )
                                .context(CreateTableSnafu)
                            })?;
                    }
                    DBEvent::QuorumReached { quorum, data } => {
                        if table_matches_filters(&quorum.table, &config.include) {
                            crate::http_client::put_batches(
                                &config.url,
                                block_num,
                                alloc::vec![(&quorum.table, data.into_inner())],
                            )
                            .context(PutBatchesSnafu)?;
                        }
                    }
                }
            }

            Ok(())
        }
    }
}
