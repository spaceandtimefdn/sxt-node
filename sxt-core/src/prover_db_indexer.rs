//! Shared items for the prover-db-indexer pallet, the node service
//! that seeds its configuration, and the producer call sites in
//! `pallet-tables` and `pallet-indexing`.

extern crate alloc;

use alloc::vec::Vec;

use codec::{Decode, Encode};

use crate::tables::TableIdentifier;

/// Offchain local-storage key holding the prover-db indexer URL.
///
/// The embedding node is expected to write the configured URL to this
/// key in OCW persistent storage before block authoring begins; the
/// prover-db-indexer pallet's offchain worker reads it to know where to
/// forward events. If the key is unset the OCW stays dormant.
pub const PROVER_DB_URL_KEY: &[u8] = b"prover_db_indexer/prover_db_url";

/// Offchain DB key prefix for per-extrinsic event payloads.
const EVENT_KEY_PREFIX: &[u8] = b"prover_db_indexer::event::";

/// Offchain DB key prefix for per-block manifests (the list of extrinsic
/// indices in a block that produced events).
const MANIFEST_KEY_PREFIX: &[u8] = b"prover_db_indexer::manifest::";

/// Compute the offchain DB key for the per-block manifest. The manifest
/// is a SCALE-encoded `Vec<u32>` of extrinsic indices in deposit order;
/// the OCW reads it to know which event sub-keys to fetch.
pub fn key_for_manifest(block: u64) -> Vec<u8> {
    let mut k = MANIFEST_KEY_PREFIX.to_vec();
    k.extend_from_slice(&block.to_be_bytes());
    k
}

/// Compute the offchain DB key for the events emitted by a single
/// extrinsic in a given block. The value at this key is a SCALE-encoded
/// `Vec<BlockEvent>` (one extrinsic may emit several `BlockEvent`s).
pub fn key_for_event(block: u64, extrinsic_index: u32) -> Vec<u8> {
    let mut k = EVENT_KEY_PREFIX.to_vec();
    k.extend_from_slice(&block.to_be_bytes());
    k.extend_from_slice(&extrinsic_index.to_be_bytes());
    k
}

/// A table-creation event.
#[derive(Encode, Decode, Debug, Clone)]
pub struct CreateEntry {
    /// Identifier of the table being created or updated.
    pub ident: TableIdentifier,
    /// DDL bytes describing the schema; forwarded to the indexer as-is.
    pub ddl: Vec<u8>,
}

/// A data-quorum event.
#[derive(Encode, Decode, Debug, Clone)]
pub struct DataEntry {
    /// Identifier of the table the data belongs to.
    pub table: TableIdentifier,
    /// Postcard-encoded `OnChainTable` bytes; forwarded to the indexer as-is.
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

/// Hook through which `pallet-tables` and `pallet-indexing` hand off
/// indexable events at extrinsic time. The runtime wires this to
/// `pallet-prover-db-indexer`; `()` is a no-op for runtimes that don't
/// run the prover-db indexer.
pub trait EventCapture {
    /// Capture the events emitted by the currently-executing extrinsic.
    /// Implementations must be cheap enough to count in the caller's
    /// declared weight (no event iteration, no on-chain unbounded reads).
    fn capture_events(events: Vec<BlockEvent>);
}

impl EventCapture for () {
    fn capture_events(_events: Vec<BlockEvent>) {}
}
