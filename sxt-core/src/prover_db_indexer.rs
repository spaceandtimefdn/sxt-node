//! Shared items for the prover-db-indexer pallet and the node service
//! that seeds its configuration.

/// Offchain local-storage key for the prover-db indexer URL.
///
/// The node writes this key from `--prover-db-url` (or the `PROVER_DB_URL`
/// env var) at startup; the prover-db-indexer pallet's offchain worker
/// reads it to know where to forward events.
pub const PROVER_DB_URL_KEY: &[u8] = b"prover_db_indexer/prover_db_url";
