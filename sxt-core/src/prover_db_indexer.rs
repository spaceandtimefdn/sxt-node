//! Shared items for the prover-db-indexer pallet, the node service
//! that seeds its configuration, and the producer call sites in
//! `pallet-tables` and `pallet-indexing`.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use codec::{Decode, Encode};

use crate::tables::{TableIdentifier, TableNamespace};

/// Offchain local-storage key holding the prover-db indexer consumer's
/// configuration. The embedding node writes a SCALE-encoded
/// [`ProverDbConsumerConfig`] under this key at startup; the OCW reads
/// it once per round. Absence of the key means "OCW is dormant".
///
/// One key for the whole consumer rather than one-per-field so the
/// "consumer is enabled" and "consumer is disabled with stale filter
/// settings" states are unrepresentable.
pub const PROVER_DB_CONFIG_KEY: &[u8] = b"prover_db_indexer/consumer_config";

/// Per-node configuration that turns this node into a prover-db
/// indexer: the upstream URL to POST forwarded events to, plus the
/// optional include set that gates which events get forwarded.
///
/// Lives in offchain local storage under [`PROVER_DB_CONFIG_KEY`].
/// `url` is non-optional: writing the config _is_ the act of enabling
/// the consumer, and there's no enabled-without-URL state to express.
/// `include` may be empty, which the OCW treats as "match every table"
/// (same default as if the operator hadn't passed any include patterns
/// at all).
#[derive(Encode, Decode, Debug, Clone, Eq, PartialEq)]
pub struct ProverDbConsumerConfig {
    /// HTTP base URL of the upstream prover-db indexer. Validated at
    /// node start; stored as raw bytes so the pallet doesn't need to
    /// pull in the `url` crate's full parsing surface.
    pub url: String,
    /// Per-node include set. Empty ⇒ forward every captured event.
    pub include: Vec<IncludeRule>,
}

/// Offchain DB key prefix for per-extrinsic event payloads. SCALE-encoded
/// as part of a `(prefix, block, ext_idx)` tuple, so the tuple structure
/// (not any trailing separator) provides the boundary between fields.
const EVENT_KEY_PREFIX: &[u8] = b"prover_db_indexer/event";

/// Offchain DB key prefix for per-block high-water-marks (the largest
/// extrinsic index in a block that produced events). See [`EVENT_KEY_PREFIX`]
/// for why there's no trailing separator.
const HIGH_WATER_KEY_PREFIX: &[u8] = b"prover_db_indexer/hwm";

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

/// A single entry in the prover-db indexer's include set. Stored only
/// in the indexer node's offchain local storage (as part of
/// [`ProverDbConsumerConfig`]); not part of on-chain state. An empty list
/// of these means "forward every event"; a non-empty list means "only
/// forward events whose table matches at least one rule".
///
/// Matches are byte-exact against the on-chain identifiers, so callers
/// that need case-insensitive matching should normalize their inputs
/// (e.g. via [`TableIdentifier::from_str_unchecked`] which uppercases)
/// before writing the rule to offchain storage.
#[derive(Encode, Decode, Debug, Clone, Eq, PartialEq)]
pub enum IncludeRule {
    /// Match every table within the given namespace.
    Namespace(TableNamespace),
    /// Match exactly one fully-qualified table identifier.
    Table(TableIdentifier),
}

/// Returns true if `table` matches at least one rule in `rules`. An
/// empty rule set is treated as "match all".
pub fn table_matches_rules(table: &TableIdentifier, rules: &[IncludeRule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    rules.iter().any(|rule| match rule {
        IncludeRule::Namespace(ns) => &table.namespace == ns,
        IncludeRule::Table(t) => t == table,
    })
}
