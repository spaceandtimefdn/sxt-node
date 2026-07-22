//! Shared items for the prover-db-indexer pallet, the node service
//! that seeds its configuration, and the producer call sites in
//! `pallet-tables` and `pallet-indexing`.

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::str::FromStr;

use codec::{Decode, Encode};
use snafu::Snafu;

use crate::tables::TableIdentifier;
use crate::IDENT_LENGTH;

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
    /// Per-node include set. An event is forwarded iff its
    /// `TableIdentifier` matches at least one filter. Empty ⇒ forward
    /// nothing; callers that want "forward everything" must include an
    /// explicit `*.*` filter.
    pub include: Vec<TableIdentifierFilter>,
}

/// Offchain DB key prefix for per-extrinsic event payloads. SCALE-encoded
/// as part of a `(prefix, block, ext_idx)` tuple, so the tuple structure
/// (not any trailing separator) provides the boundary between fields.
const EVENT_KEY_PREFIX: &[u8] = b"prover_db_indexer/event";

/// Offchain DB key prefix for per-block high-water-marks (the largest
/// extrinsic index in a block that produced events). See [`EVENT_KEY_PREFIX`]
/// for why there's no trailing separator.
const HIGH_WATER_KEY_PREFIX: &[u8] = b"prover_db_indexer/high_water_mark";

/// Compute the offchain DB key for a block's high-water-mark. The value
/// at this key is a SCALE-encoded `u32`: the largest `extrinsic_index`
/// in the block that called `EventCapture::capture_events`. The OCW
/// reads it to know how far to probe `key_for_event(block, 0..=high_water_mark)`.
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

impl BlockEvent<'_> {
    /// Returns the table identifier associated with this event.
    pub fn table(&self) -> &TableIdentifier {
        match self {
            Self::Create(entry) => entry.ident.as_ref(),
            Self::Drop(ident) => ident.as_ref(),
            Self::Insert(entry) => entry.table.as_ref(),
        }
    }
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

/// Filter for one side (namespace or name) of a `TableIdentifier`.
/// Either matches every identifier (`Wildcard`) or matches exactly one
/// byte sequence (`Ident`). `Ident` values are stored uppercased so
/// matches against the on-chain canonical form are byte-exact without
/// per-callsite vigilance.
#[derive(Encode, Decode, Debug, Clone, Eq, PartialEq)]
pub enum IdentFilter {
    /// Matches any identifier on this side of the dot.
    Wildcard,
    /// Matches an identifier whose bytes equal these.
    Ident(String),
}

impl IdentFilter {
    /// Returns true if `bytes` (the on-chain canonical, uppercased form)
    /// matches this filter.
    pub fn matches(&self, bytes: &[u8]) -> bool {
        match self {
            Self::Wildcard => true,
            Self::Ident(s) => s.as_bytes() == bytes,
        }
    }
}

/// `FromStr` parse error for [`IdentFilter`].
#[derive(Debug, Snafu)]
pub enum IdentFilterParseError {
    /// The input was empty (neither `*` nor a non-empty identifier).
    #[snafu(display("identifier filter is empty"))]
    Empty,
    /// The identifier exceeded the on-chain identifier length cap and so
    /// could never match a captured table identifier.
    #[snafu(display("identifier filter exceeds the {max}-byte on-chain length cap"))]
    TooLong {
        /// The cap, in bytes (`IDENT_LENGTH`).
        max: u32,
    },
}

impl FromStr for IdentFilter {
    type Err = IdentFilterParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "*" {
            return Ok(Self::Wildcard);
        }
        if s.is_empty() {
            return Err(IdentFilterParseError::Empty);
        }
        let upper = s.to_uppercase();
        if upper.len() as u32 > IDENT_LENGTH {
            return Err(IdentFilterParseError::TooLong { max: IDENT_LENGTH });
        }
        Ok(Self::Ident(upper))
    }
}

/// Filter against a fully-qualified [`TableIdentifier`]. Matches when
/// both sides match — so `*.*` is "everything", `NS.*` is "every table
/// in NS", `NS.NAME` is exact, and `*.NAME` is "any namespace, this
/// name".
///
/// Stored only in the indexer node's offchain local storage (as the
/// `include` field of [`ProverDbConsumerConfig`]); not part of on-chain
/// state. An empty list matches nothing — see [`table_matches_filters`].
#[derive(Encode, Decode, Debug, Clone, Eq, PartialEq)]
pub struct TableIdentifierFilter {
    /// Filter applied to `ident.namespace`.
    pub namespace_filter: IdentFilter,
    /// Filter applied to `ident.name`.
    pub name_filter: IdentFilter,
}

impl TableIdentifierFilter {
    /// Returns true if `ident` passes both sides of this filter.
    pub fn matches(&self, ident: &TableIdentifier) -> bool {
        self.namespace_filter.matches(&ident.namespace) && self.name_filter.matches(&ident.name)
    }
}

/// `FromStr` parse error for [`TableIdentifierFilter`].
#[derive(Debug, Snafu)]
pub enum TableIdentifierFilterParseError {
    /// The input didn't contain exactly one `.` separator.
    #[snafu(display("expected 'NAMESPACE.NAME' form with exactly one dot, got '{input}'"))]
    MalformedShape {
        /// The original input string.
        input: String,
    },
    /// The namespace side failed to parse as an [`IdentFilter`].
    #[snafu(display("invalid namespace filter: {source}"))]
    Namespace {
        /// Underlying [`IdentFilter`] parse error.
        source: IdentFilterParseError,
    },
    /// The name side failed to parse as an [`IdentFilter`].
    #[snafu(display("invalid name filter: {source}"))]
    Name {
        /// Underlying [`IdentFilter`] parse error.
        source: IdentFilterParseError,
    },
}

impl FromStr for TableIdentifierFilter {
    type Err = TableIdentifierFilterParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((ns, name)) = s.split_once('.') else {
            return Err(TableIdentifierFilterParseError::MalformedShape {
                input: s.to_string(),
            });
        };
        if s.matches('.').count() != 1 {
            return Err(TableIdentifierFilterParseError::MalformedShape {
                input: s.to_string(),
            });
        }
        let namespace_filter = ns
            .parse()
            .map_err(|source| TableIdentifierFilterParseError::Namespace { source })?;
        let name_filter = name
            .parse()
            .map_err(|source| TableIdentifierFilterParseError::Name { source })?;
        Ok(Self {
            namespace_filter,
            name_filter,
        })
    }
}

/// Returns true if `table` matches at least one filter in `filters`. An
/// empty filter set matches nothing — callers that want "match all"
/// must pass an explicit `*.*` filter. This makes "no filters
/// configured" and "match-all configured" distinct, addressable states.
pub fn table_matches_filters(table: &TableIdentifier, filters: &[TableIdentifierFilter]) -> bool {
    filters.iter().any(|f| f.matches(table))
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    fn ident(name: &str, namespace: &str) -> TableIdentifier {
        TableIdentifier::from_str_unchecked(name, namespace)
    }

    // ── IdentFilter::matches ─────────────────────────────────────────

    #[test]
    fn ident_filter_wildcard_matches_anything() {
        assert!(IdentFilter::Wildcard.matches(b""));
        assert!(IdentFilter::Wildcard.matches(b"ANYTHING"));
    }

    #[test]
    fn ident_filter_ident_matches_exact_bytes_only() {
        let f = IdentFilter::Ident("FOO".into());
        assert!(f.matches(b"FOO"));
        assert!(!f.matches(b"foo")); // case-sensitive at match time
        assert!(!f.matches(b"FOOBAR"));
        assert!(!f.matches(b"FO"));
        assert!(!f.matches(b""));
    }

    // ── IdentFilter::FromStr ─────────────────────────────────────────

    #[test]
    fn ident_filter_parses_star_as_wildcard() {
        assert_eq!("*".parse::<IdentFilter>().unwrap(), IdentFilter::Wildcard);
    }

    #[test]
    fn ident_filter_parses_ident_uppercased() {
        assert_eq!(
            "foo".parse::<IdentFilter>().unwrap(),
            IdentFilter::Ident("FOO".into()),
        );
        assert_eq!(
            "MIXEDcase".parse::<IdentFilter>().unwrap(),
            IdentFilter::Ident("MIXEDCASE".into()),
        );
    }

    #[test]
    fn ident_filter_rejects_empty_string() {
        assert!(matches!(
            "".parse::<IdentFilter>(),
            Err(IdentFilterParseError::Empty),
        ));
    }

    #[test]
    fn ident_filter_rejects_over_length_input() {
        // IDENT_LENGTH + 1 ASCII bytes — uppercase is a no-op so the
        // post-uppercase length is also IDENT_LENGTH + 1.
        let too_long: alloc::string::String =
            core::iter::repeat_n('A', IDENT_LENGTH as usize + 1).collect();
        assert!(matches!(
            too_long.parse::<IdentFilter>(),
            Err(IdentFilterParseError::TooLong { max }) if max == IDENT_LENGTH,
        ));
    }

    // ── TableIdentifierFilter::matches ───────────────────────────────

    #[test]
    fn table_filter_matches_only_when_both_sides_match() {
        let table = ident("T1", "ALPHA");

        let alpha_t1: TableIdentifierFilter = "ALPHA.T1".parse().unwrap();
        let alpha_star: TableIdentifierFilter = "ALPHA.*".parse().unwrap();
        let star_t1: TableIdentifierFilter = "*.T1".parse().unwrap();
        let star_star: TableIdentifierFilter = "*.*".parse().unwrap();
        let beta_t1: TableIdentifierFilter = "BETA.T1".parse().unwrap();
        let alpha_other: TableIdentifierFilter = "ALPHA.OTHER".parse().unwrap();

        assert!(alpha_t1.matches(&table));
        assert!(alpha_star.matches(&table));
        assert!(star_t1.matches(&table));
        assert!(star_star.matches(&table));
        assert!(!beta_t1.matches(&table));
        assert!(!alpha_other.matches(&table));
    }

    // ── TableIdentifierFilter::FromStr ───────────────────────────────

    #[test]
    fn table_filter_parses_all_four_shapes() {
        let full: TableIdentifierFilter = "ns.name".parse().unwrap();
        assert!(matches!(full.namespace_filter, IdentFilter::Ident(ref s) if s == "NS"));
        assert!(matches!(full.name_filter, IdentFilter::Ident(ref s) if s == "NAME"));

        let ns_star: TableIdentifierFilter = "ns.*".parse().unwrap();
        assert!(matches!(ns_star.namespace_filter, IdentFilter::Ident(ref s) if s == "NS"));
        assert!(matches!(ns_star.name_filter, IdentFilter::Wildcard));

        let star_name: TableIdentifierFilter = "*.name".parse().unwrap();
        assert!(matches!(star_name.namespace_filter, IdentFilter::Wildcard));
        assert!(matches!(star_name.name_filter, IdentFilter::Ident(ref s) if s == "NAME"));

        let star_star: TableIdentifierFilter = "*.*".parse().unwrap();
        assert!(matches!(star_star.namespace_filter, IdentFilter::Wildcard));
        assert!(matches!(star_star.name_filter, IdentFilter::Wildcard));
    }

    #[test]
    fn table_filter_rejects_missing_dot() {
        assert!(matches!(
            "namename".parse::<TableIdentifierFilter>(),
            Err(TableIdentifierFilterParseError::MalformedShape { .. }),
        ));
    }

    #[test]
    fn table_filter_rejects_more_than_one_dot() {
        assert!(matches!(
            "a.b.c".parse::<TableIdentifierFilter>(),
            Err(TableIdentifierFilterParseError::MalformedShape { .. }),
        ));
    }

    #[test]
    fn table_filter_propagates_empty_side_error() {
        assert!(matches!(
            ".name".parse::<TableIdentifierFilter>(),
            Err(TableIdentifierFilterParseError::Namespace {
                source: IdentFilterParseError::Empty,
            }),
        ));
        assert!(matches!(
            "ns.".parse::<TableIdentifierFilter>(),
            Err(TableIdentifierFilterParseError::Name {
                source: IdentFilterParseError::Empty,
            }),
        ));
    }

    // ── table_matches_filters ────────────────────────────────────────

    #[test]
    fn empty_filter_set_matches_nothing() {
        let table = ident("T1", "ALPHA");
        assert!(!table_matches_filters(&table, &[]));
    }

    #[test]
    fn explicit_wildcard_filter_matches_everything() {
        let table = ident("T1", "ALPHA");
        let star: TableIdentifierFilter = "*.*".parse().unwrap();
        assert!(table_matches_filters(&table, core::slice::from_ref(&star)));
    }

    #[test]
    fn non_empty_filter_set_requires_at_least_one_match() {
        let table = ident("T1", "ALPHA");
        let filters = vec![
            "BETA.*".parse().unwrap(),
            "GAMMA.T1".parse().unwrap(),
            "ALPHA.T1".parse().unwrap(), // this one matches
        ];
        assert!(table_matches_filters(&table, &filters));

        let no_match = vec!["BETA.*".parse().unwrap(), "GAMMA.T1".parse().unwrap()];
        assert!(!table_matches_filters(&table, &no_match));
    }
}
