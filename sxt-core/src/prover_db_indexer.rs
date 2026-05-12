//! Shared items for the prover-db-indexer pallet, the node service
//! that seeds its configuration, and the producer call sites in
//! `pallet-tables` and `pallet-indexing`.

extern crate alloc;

use alloc::borrow::Cow;
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
const EVENT_KEY_PREFIX: &[u8] = b"prover_db_indexer/event/";

/// Offchain DB key prefix for per-block high-water-marks (the largest
/// extrinsic index in a block that produced events).
const HIGH_WATER_KEY_PREFIX: &[u8] = b"prover_db_indexer/hwm/";

/// Compute the offchain DB key for a block's high-water-mark. The value
/// at this key is a SCALE-encoded `u32`: the largest `extrinsic_index`
/// in the block that called `EventCapture::capture_events`. The OCW
/// reads it to know how far to probe `key_for_event(block, 0..=hwm)`.
/// Absence of this key means the block had zero captured events.
pub fn key_for_high_water(block: u64) -> Vec<u8> {
    (HIGH_WATER_KEY_PREFIX, block).encode()
}

/// Compute the offchain DB key for the events emitted by a single
/// extrinsic in a given block. The value at this key is a SCALE-encoded
/// `Vec<BlockEvent>` (one extrinsic may emit several `BlockEvent`s).
pub fn key_for_event(block: u64, extrinsic_index: u32) -> Vec<u8> {
    (EVENT_KEY_PREFIX, block, extrinsic_index).encode()
}

/// A table-creation event.
///
/// Fields are `Cow` so producer call sites can pass borrowed references
/// (the captured event is encoded immediately and never outlives the
/// caller's stack frame) while the OCW consumer decodes into `Cow::Owned`
/// transparently — SCALE encodes `Cow<'_, T>` identically to `T`.
#[derive(Encode, Decode, Debug, Clone)]
pub struct CreateEntry<'a> {
    /// Identifier of the table being created or updated.
    pub ident: Cow<'a, TableIdentifier>,
    /// DDL bytes describing the schema; forwarded to the indexer as-is.
    pub ddl: Cow<'a, [u8]>,
}

/// A row-insert event triggered by a data quorum. See [`CreateEntry`]
/// for the `Cow` rationale.
#[derive(Encode, Decode, Debug, Clone)]
pub struct InsertEntry<'a> {
    /// Identifier of the table the rows belong to.
    pub table: Cow<'a, TableIdentifier>,
    /// Postcard-encoded `OnChainTable` bytes; forwarded to the indexer as-is.
    pub data: Cow<'a, [u8]>,
}

/// A single event captured during block execution. Stored in the order
/// events were deposited so the OCW replays them in the correct sequence.
/// Variant names mirror the corresponding indexer DB operations.
#[derive(Encode, Decode, Debug, Clone)]
pub enum BlockEvent<'a> {
    /// Table created or schema updated.
    Create(CreateEntry<'a>),
    /// Table dropped.
    Drop(Cow<'a, TableIdentifier>),
    /// Rows inserted (data quorum reached).
    Insert(InsertEntry<'a>),
}

/// Hook through which `pallet-tables` and `pallet-indexing` hand off
/// indexable events at extrinsic time. The runtime wires this to
/// `pallet-prover-db-indexer`; `()` is a no-op for runtimes that don't
/// run the prover-db indexer.
///
/// Call at most once per extrinsic: the implementation keys the offchain
/// blob by `extrinsic_index`, so a second call from the same extrinsic
/// would overwrite the first.
pub trait EventCapture {
    /// Capture the events emitted by the currently-executing extrinsic.
    /// Implementations must be cheap enough to count in the caller's
    /// declared weight.
    fn capture_events(events: Vec<BlockEvent<'_>>);
}

impl EventCapture for () {
    fn capture_events(_events: Vec<BlockEvent<'_>>) {}
}
