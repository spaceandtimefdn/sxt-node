//! Shared items for the prover-db-indexer pallet, the node service
//! that supplies its configuration, and the producer call sites in
//! `pallet-tables` and `pallet-indexing`.

use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::num::ParseIntError;
use core::str::FromStr;

use codec::{Decode, Encode};
use snafu::{OptionExt, ResultExt, Snafu};
use url::Url;

use crate::tables::TableIdentifier;
use crate::IDENT_LENGTH;

/// Config key holding the prover-db indexer's target URL.
pub const PROVER_DB_CONFIG_URL_KEY: &str = "prover_db_indexer/url";

/// Config key holding the comma-separated `NAMESPACE.NAME` include filters.
pub const PROVER_DB_CONFIG_INCLUDE_KEY: &str = "prover_db_indexer/include";

/// Config key overriding [`DEFAULT_MAX_BLOCKS_PER_INVOCATION`].
pub const MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY: &str =
    "prover_db_indexer/max_blocks_per_invocation";

/// Default cap on blocks walked per OCW invocation.
pub const DEFAULT_MAX_BLOCKS_PER_INVOCATION: usize = 100;

/// Config key overriding [`DEFAULT_OCW_LOCK_DEADLINE_MS`].
pub const OCW_LOCK_DEADLINE_MS_CONFIG_KEY: &str = "prover_db_indexer/ocw_lock_deadline_ms";

/// Default OCW storage lock deadline, in milliseconds.
pub const DEFAULT_OCW_LOCK_DEADLINE_MS: u64 = 120_000;

/// Runtime configuration for the prover-db OCW consumer.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProverDbConsumerConfig {
    /// HTTP base URL of the upstream prover-db indexer.
    pub url: Url,
    /// Per-node include set. An event is forwarded iff its
    /// `TableIdentifier` matches at least one filter. Empty ⇒ forward
    /// nothing; callers that want "forward everything" must include an
    /// explicit `*.*` filter.
    pub include: Vec<TableIdentifierFilter>,
    /// Cap on blocks walked per OCW invocation.
    pub max_blocks_per_invocation: usize,
    /// OCW storage lock deadline, in milliseconds.
    pub ocw_lock_deadline_ms: u64,
}

/// Errors from building a [`ProverDbConsumerConfig`] out of raw config strings.
#[derive(Debug, Snafu)]
pub enum ProverDbConsumerConfigError {
    /// Config externality not registered.
    #[snafu(display("config externality not registered"))]
    NotRegistered,
    /// [`PROVER_DB_CONFIG_URL_KEY`] is not set in the configuration.
    #[snafu(display("configuration key '{PROVER_DB_CONFIG_URL_KEY}' is not set"))]
    MissingUrl,
    /// The configured URL failed to parse.
    #[snafu(display(
        "failed to parse value provided for '{PROVER_DB_CONFIG_URL_KEY}' key: {error}"
    ))]
    ParseUrl {
        /// Underlying parse error from the `url` crate.
        error: url::ParseError,
    },
    /// The configured include filter set failed to parse.
    #[snafu(display(
        "failed to parse value provided for '{PROVER_DB_CONFIG_INCLUDE_KEY}' key: {source}"
    ))]
    ParseFilter {
        /// Underlying [`TableIdentifierFilter`] parse error.
        source: TableIdentifierFilterParseError,
    },
    /// [`MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY`] failed to parse as a `usize`.
    #[snafu(display(
        "failed to parse value provided for '{MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY}' key: {source}"
    ))]
    ParseMaxBlocks {
        /// Underlying integer parse error.
        source: ParseIntError,
    },
    /// [`OCW_LOCK_DEADLINE_MS_CONFIG_KEY`] failed to parse as a `u64`.
    #[snafu(display(
        "failed to parse value provided for '{OCW_LOCK_DEADLINE_MS_CONFIG_KEY}' key: {source}"
    ))]
    ParseLockDeadline {
        /// Underlying integer parse error.
        source: ParseIntError,
    },
}

impl ProverDbConsumerConfig {
    /// Builds a config by looking up each setting via `get`, where
    /// `get(key)` returns `None` if `key` isn't registered at all, or
    /// `Some(None)` if it's registered but unset.
    pub fn try_from_map<F: Fn(&str) -> Option<Option<String>>>(
        get: F,
    ) -> Result<Self, ProverDbConsumerConfigError> {
        let url = get(PROVER_DB_CONFIG_URL_KEY)
            .context(NotRegisteredSnafu)?
            .context(MissingUrlSnafu)?
            .parse()
            .map_err(|error| ProverDbConsumerConfigError::ParseUrl { error })?;
        let include = get(PROVER_DB_CONFIG_INCLUDE_KEY)
            .context(NotRegisteredSnafu)?
            .map(|s| {
                if s.is_empty() {
                    Ok(Vec::new())
                } else {
                    s.split(',').map(str::parse).collect::<Result<_, _>>()
                }
            })
            .transpose()
            .context(ParseFilterSnafu)?
            .unwrap_or_default();
        let max_blocks_per_invocation = get(MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY)
            .context(NotRegisteredSnafu)?
            .map(|s| s.parse())
            .transpose()
            .context(ParseMaxBlocksSnafu)?
            .unwrap_or(DEFAULT_MAX_BLOCKS_PER_INVOCATION);
        let ocw_lock_deadline_ms = get(OCW_LOCK_DEADLINE_MS_CONFIG_KEY)
            .context(NotRegisteredSnafu)?
            .map(|s| s.parse())
            .transpose()
            .context(ParseLockDeadlineSnafu)?
            .unwrap_or(DEFAULT_OCW_LOCK_DEADLINE_MS);
        Ok(ProverDbConsumerConfig {
            url,
            include,
            max_blocks_per_invocation,
            ocw_lock_deadline_ms,
        })
    }
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

    // ── ProverDbConsumerConfig::try_from_map ─────────────────────────

    /// Builds a `get` closure from an explicit key list. A key absent
    /// from `entries` is "not registered" (`None`); a key present with
    /// value `None` is "registered but unset" (`Some(None)`).
    fn make_get(
        entries: &'static [(&'static str, Option<&'static str>)],
    ) -> impl Fn(&str) -> Option<Option<String>> {
        move |key| {
            entries
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.map(String::from))
        }
    }

    #[test]
    fn try_from_map_fails_when_not_registered() {
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[])),
            Err(ProverDbConsumerConfigError::NotRegistered),
        ));
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[(
                PROVER_DB_CONFIG_URL_KEY,
                Some("http://example.com"),
            )])),
            Err(ProverDbConsumerConfigError::NotRegistered),
        ));
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[
                (PROVER_DB_CONFIG_URL_KEY, Some("http://example.com")),
                (PROVER_DB_CONFIG_INCLUDE_KEY, None),
            ])),
            Err(ProverDbConsumerConfigError::NotRegistered),
        ));
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[
                (PROVER_DB_CONFIG_URL_KEY, Some("http://example.com")),
                (PROVER_DB_CONFIG_INCLUDE_KEY, None),
                (MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY, None),
            ])),
            Err(ProverDbConsumerConfigError::NotRegistered),
        ));
    }

    #[test]
    fn try_from_map_fails_when_url_missing() {
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[(PROVER_DB_CONFIG_URL_KEY, None)])),
            Err(ProverDbConsumerConfigError::MissingUrl),
        ));
    }

    #[test]
    fn try_from_map_fails_when_values_fail_to_parse() {
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[(
                PROVER_DB_CONFIG_URL_KEY,
                Some("not a url"),
            )])),
            Err(ProverDbConsumerConfigError::ParseUrl { .. }),
        ));
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[
                (PROVER_DB_CONFIG_URL_KEY, Some("http://example.com")),
                (PROVER_DB_CONFIG_INCLUDE_KEY, Some("not-a-filter")),
            ])),
            Err(ProverDbConsumerConfigError::ParseFilter { .. }),
        ));
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[
                (PROVER_DB_CONFIG_URL_KEY, Some("http://example.com")),
                (PROVER_DB_CONFIG_INCLUDE_KEY, None),
                (MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY, Some("not-a-number")),
            ])),
            Err(ProverDbConsumerConfigError::ParseMaxBlocks { .. }),
        ));
        assert!(matches!(
            ProverDbConsumerConfig::try_from_map(make_get(&[
                (PROVER_DB_CONFIG_URL_KEY, Some("http://example.com")),
                (PROVER_DB_CONFIG_INCLUDE_KEY, None),
                (MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY, None),
                (OCW_LOCK_DEADLINE_MS_CONFIG_KEY, Some("not-a-number")),
            ])),
            Err(ProverDbConsumerConfigError::ParseLockDeadline { .. }),
        ));
    }

    #[test]
    fn try_from_map_succeeds_with_provided_values() {
        let config = ProverDbConsumerConfig::try_from_map(make_get(&[
            (PROVER_DB_CONFIG_URL_KEY, Some("http://example.com")),
            (PROVER_DB_CONFIG_INCLUDE_KEY, Some("ALPHA.T1,*.*")),
            (MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY, Some("42")),
            (OCW_LOCK_DEADLINE_MS_CONFIG_KEY, Some("9999")),
        ]))
        .unwrap();
        assert_eq!(config.url, Url::parse("http://example.com").unwrap());
        assert_eq!(
            config.include,
            vec!["ALPHA.T1".parse().unwrap(), "*.*".parse().unwrap()],
        );
        assert_eq!(config.max_blocks_per_invocation, 42);
        assert_eq!(config.ocw_lock_deadline_ms, 9999);
    }

    #[test]
    fn try_from_map_succeeds_with_defaults_when_unset() {
        let config = ProverDbConsumerConfig::try_from_map(make_get(&[
            (PROVER_DB_CONFIG_URL_KEY, Some("http://example.com")),
            (PROVER_DB_CONFIG_INCLUDE_KEY, None),
            (MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY, None),
            (OCW_LOCK_DEADLINE_MS_CONFIG_KEY, None),
        ]))
        .unwrap();
        assert_eq!(config.include, Vec::new());
        assert_eq!(
            config.max_blocks_per_invocation,
            DEFAULT_MAX_BLOCKS_PER_INVOCATION,
        );
        assert_eq!(config.ocw_lock_deadline_ms, DEFAULT_OCW_LOCK_DEADLINE_MS);
    }

    #[test]
    fn try_from_map_succeeds_with_empty_include() {
        let config = ProverDbConsumerConfig::try_from_map(make_get(&[
            (PROVER_DB_CONFIG_URL_KEY, Some("http://example.com")),
            (PROVER_DB_CONFIG_INCLUDE_KEY, Some("")),
            (MAX_BLOCKS_PER_INVOCATION_CONFIG_KEY, None),
            (OCW_LOCK_DEADLINE_MS_CONFIG_KEY, None),
        ]))
        .unwrap();
        assert_eq!(config.include, Vec::new());
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
