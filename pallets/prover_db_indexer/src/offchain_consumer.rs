//! OCW-only helpers for reading and clearing the offchain DB entries
//! the producer wrote. Producer-side helpers (key construction, types)
//! live in `sxt_core::prover_db_indexer`; this module wraps the OCW host
//! functions (`local_storage_get` / `local_storage_clear`) so the
//! consumer code in `lib.rs` doesn't repeat the boilerplate.

use alloc::vec::Vec;

use codec::Decode;
use polkadot_sdk::sp_core::offchain::StorageKind;
use sxt_core::prover_db_indexer::{key_for_event, key_for_high_water, BlockEvent};

/// Read the per-block high-water-mark. `None` means the block had no
/// captured events.
pub fn read_high_water(block: u64) -> Option<u32> {
    let raw = polkadot_sdk::sp_io::offchain::local_storage_get(
        StorageKind::PERSISTENT,
        &key_for_high_water(block),
    )?;
    u32::decode(&mut &raw[..]).ok()
}

/// Read the events emitted by a single extrinsic in a given block.
/// `None` means that extrinsic did not call `EventCapture::capture_events`.
pub fn read_events(block: u64, extrinsic_index: u32) -> Option<Vec<BlockEvent>> {
    let raw = polkadot_sdk::sp_io::offchain::local_storage_get(
        StorageKind::PERSISTENT,
        &key_for_event(block, extrinsic_index),
    )?;
    Vec::<BlockEvent>::decode(&mut &raw[..]).ok()
}

/// Delete the per-block high-water-mark.
pub fn clear_high_water(block: u64) {
    polkadot_sdk::sp_io::offchain::local_storage_clear(
        StorageKind::PERSISTENT,
        &key_for_high_water(block),
    );
}

/// Delete the per-extrinsic event payload.
pub fn clear_events(block: u64, extrinsic_index: u32) {
    polkadot_sdk::sp_io::offchain::local_storage_clear(
        StorageKind::PERSISTENT,
        &key_for_event(block, extrinsic_index),
    );
}
