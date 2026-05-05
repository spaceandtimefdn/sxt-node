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
//!   appends the extrinsic index to a small per-block manifest at
//!   `key_for_manifest(block)`. The manifest is accumulated in on-chain
//!   storage so we can mirror the cumulative list to the offchain DB on
//!   every capture without reading the offchain DB mid-block.
//!
//! **Consumer** (`offchain_worker`, fires at chain tip only):
//!   For each block in `cursor+1..=current`, reads the manifest, walks
//!   the populated extrinsic indices, forwards each entry via
//!   HTTP+protobuf, checkpoints on the server, deletes consumed
//!   entries, advances cursor.
//!
//! This PR ships the producer half. The consumer (HTTP forwarding) is
//! added in a follow-up PR; until that lands, `offchain_worker` is a
//! no-op stub and any payload the producer writes simply sits in the
//! offchain DB.
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
//! ## Why on-chain accumulation for the manifest only?
//!
//! Each capture call writes its own (potentially large) event payload
//! directly to the offchain DB — those bytes never touch on-chain
//! storage. Only the small list of extrinsic indices is accumulated
//! on-chain so the OCW (which cannot enumerate offchain keys) knows
//! which sub-keys to fetch. The manifest is bounded by
//! [`MaxEventsPerBlock`] and reset each block in `on_initialize`.
//!
//! [`EventCapture`]: sxt_core::prover_db_indexer::EventCapture
//! [`MaxEventsPerBlock`]: pallet::Config::MaxEventsPerBlock

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod native_pallet;

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
    use polkadot_sdk::frame_system::pallet_prelude::*;
    use sxt_core::prover_db_indexer::{key_for_event, key_for_manifest, BlockEvent, EventCapture};

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: polkadot_sdk::frame_system::Config {
        /// The runtime's overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>;

        /// Upper bound on the number of distinct extrinsics per block
        /// that may emit indexable events. Sized to comfortably exceed
        /// realistic block compositions; if hit, additional events for
        /// the same block are dropped (and a warning logged).
        type MaxEventsPerBlock: Get<u32>;
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

    /// Per-block accumulator of extrinsic indices that emitted indexable
    /// events. Reset to empty in `on_initialize`. Mirrored to the
    /// offchain DB at `key_for_manifest(block)` on every capture so the
    /// OCW knows which sub-keys to fetch.
    #[pallet::storage]
    pub type CurrentBlockManifest<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BoundedVec<u32, <T as Config<I>>::MaxEventsPerBlock>, ValueQuery>;

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
        fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
            // Constant-cost reset: clear the manifest accumulator so each
            // block starts fresh. The offchain blob written under the
            // previous block's manifest key remains readable by the OCW.
            CurrentBlockManifest::<T, I>::kill();
            T::DbWeight::get().writes(1)
        }

        // ─── CONSUMER ───────────────────────────────────────────────────
        // Stubbed in this PR; HTTP forwarding logic lands in the follow-up.
        fn offchain_worker(_block_number: BlockNumberFor<T>) {}
    }

    impl<T: Config<I>, I: 'static> EventCapture for Pallet<T, I> {
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

            // Write the per-extrinsic event payload to the offchain DB.
            polkadot_sdk::sp_io::offchain_index::set(
                &key_for_event(block, ext_idx),
                &events.encode(),
            );

            // Append this extrinsic index to the on-chain manifest, then
            // mirror the manifest to the offchain DB so the OCW can read
            // it. If the bounded vec is full we drop the new index; the
            // event payload is still written, but the OCW won't discover
            // it. In practice the bound is well above realistic block
            // compositions; hitting it indicates a misconfigured runtime.
            CurrentBlockManifest::<T, I>::mutate(|manifest| {
                if manifest.try_push(ext_idx).is_err() {
                    log::warn!(
                        target: "prover_db_indexer",
                        "manifest full at block {} (cap = {}); dropping extrinsic index {}",
                        block,
                        T::MaxEventsPerBlock::get(),
                        ext_idx,
                    );
                    return;
                }
                polkadot_sdk::sp_io::offchain_index::set(
                    &key_for_manifest(block),
                    &manifest.to_vec().encode(),
                );
            });
        }
    }
}
