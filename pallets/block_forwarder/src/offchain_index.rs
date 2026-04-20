//! Block-indexed offchain storage for durable event forwarding.
//!
//! `on_finalize` writes a `BlockIndex` entry per block (if non-empty).
//! The OCW reads, forwards, and deletes consumed entries.
//! Events are stored in deposit order to preserve intra-block semantics.

use alloc::vec::Vec;

use codec::{Decode, Encode};
use sxt_core::tables::TableIdentifier;

/// Key prefix for block-indexed entries in the offchain DB.
const PREFIX: &[u8] = b"block_forwarder::block::";

/// Compute the offchain DB key for a given block number.
pub fn key_for_block(block: u64) -> Vec<u8> {
    let mut k = PREFIX.to_vec();
    k.extend_from_slice(&block.to_be_bytes());
    k
}

/// A table-creation event.
#[derive(Encode, Decode, Debug, Clone)]
pub struct CreateEntry {
    pub ident: TableIdentifier,
    pub ddl: Vec<u8>,
}

/// A data-quorum event.
#[derive(Encode, Decode, Debug, Clone)]
pub struct DataEntry {
    pub table: TableIdentifier,
    pub data: Vec<u8>,
}

/// A single event captured during block execution. Stored in the order
/// events were deposited so the OCW replays them in the correct sequence.
#[derive(Encode, Decode, Debug, Clone)]
pub enum BlockEvent {
    /// Table created or schema updated.
    Create(CreateEntry),
    /// Table dropped.
    Drop(TableIdentifier),
    /// Data quorum reached (finalized row data).
    Data(DataEntry),
}

/// All relevant events from a single block, in deposit order.
#[derive(Encode, Decode, Debug, Clone, Default)]
pub struct BlockIndex {
    pub events: Vec<BlockEvent>,
}

impl BlockIndex {
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// Write a BlockIndex to the offchain DB. Called from `on_finalize`.
pub fn write(block: u64, index: &BlockIndex) {
    let key = key_for_block(block);
    polkadot_sdk::sp_io::offchain_index::set(&key, &index.encode());
}

/// Read a BlockIndex from the offchain DB. Called from the OCW.
pub fn read(block: u64) -> Option<BlockIndex> {
    let key = key_for_block(block);
    let raw = polkadot_sdk::sp_io::offchain::local_storage_get(
        polkadot_sdk::sp_core::offchain::StorageKind::PERSISTENT,
        &key,
    )?;
    BlockIndex::decode(&mut &raw[..]).ok()
}

/// Delete a consumed entry. Called from the OCW after forwarding.
pub fn clear(block: u64) {
    let key = key_for_block(block);
    polkadot_sdk::sp_io::offchain::local_storage_clear(
        polkadot_sdk::sp_core::offchain::StorageKind::PERSISTENT,
        &key,
    );
}
