//! # Prover-Db Indexer Pallet
//!
//! Forwards table lifecycle and data events to an external prover-db
//! indexer (HTTP) using protobuf-over-HTTP.
//!
//! ## Architecture
//!
//! **Producer** (extrinsic-time, via the [`EventCapture`] trait):
//!   `pallet-tables` and `pallet-indexing` call
//!   `T::EventCapture::capture_events(...)` immediately after depositing
//!   their schema/quorum events. The pallet writes the per-extrinsic
//!   payload to the offchain DB at `key_for_event(block, ext_idx)` and
//!   overwrites a per-block "high-water-mark" key at
//!   `key_for_high_water(block)` carrying the largest `ext_idx` that
//!   captured anything in this block. No on-chain state is needed —
//!   `extrinsic_index` is already available to the runtime, and
//!   absence of the high-water key means the block had no captures.
//!
//! **Consumer** (`offchain_worker`, fires at chain tip only):
//!   For each block in `cursor+1..=current`, reads the high-water-mark.
//!   If absent, the block had zero captures and is skipped. Otherwise
//!   probes `key_for_event(block, 0..=hwm)`, forwards each present
//!   entry via HTTP+protobuf, checkpoints on the server, deletes
//!   consumed entries, advances cursor.
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

pub use pallet::*;
/// Re-export of the canonical offchain local-storage key (defined in `sxt-core`),
/// kept here so existing call sites that reach for `pallet_prover_db_indexer::PROVER_DB_URL_KEY`
/// keep working.
pub use sxt_core::prover_db_indexer::PROVER_DB_URL_KEY;

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
    use sxt_core::prover_db_indexer::{
        key_for_event,
        key_for_high_water,
        BlockEvent,
        EventCapture,
    };

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config {
        /// The runtime's overarching event type.
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>;
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

    impl<T: Config> EventCapture for Pallet<T> {
        fn capture_events(events: Vec<BlockEvent>) {
            if events.is_empty() {
                return;
            }

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
}
