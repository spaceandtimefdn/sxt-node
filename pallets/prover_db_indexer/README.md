# Prover-Db Indexer Pallet

Forwards table lifecycle and data events to an external prover-db
indexer (HTTP) using protobuf-over-HTTP.

The `offchain_worker` hook (fires at chain tip only) asks the
indexer server for its last checkpoint sequence number, walks blocks
`cursor+1..=current` (capped per invocation), and for each block
reads `pallet-tables` / `pallet-indexing` events straight from the
node's client via [`crate::db_events::db_events_at`]. Matching
events forward via HTTP+protobuf:
- `pallet_tables::SchemaUpdated` → `create_table`
- `pallet_tables::TableDropped` → `drop_table`
- `pallet_indexing::QuorumReached` → `put_batches`

Gated by `prover_db_indexer/enabled` (default `false`). Once enabled,
the consumer is configured via node-supplied config keys — see
[`sxt_core::prover_db_indexer::ProverDbConsumerConfig`] for the
indexer URL, include filters, block-per-invocation cap, and OCW
lock deadline.