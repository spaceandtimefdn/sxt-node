# pallet-prover-db-indexer

Captures table lifecycle and data events at extrinsic time and persists
them to the offchain DB, so a future consumer can ship them to an
external prover-db indexer. This crate currently ships the **producer
half only**; the offchain-worker consumer is added in a follow-up PR.

`pallet-tables` and `pallet-indexing` call into this pallet via the
`EventCapture` trait, immediately after depositing the relevant event.
The producer's cost is paid by the calling extrinsic's declared weight,
not by an `on_finalize` hook.

Captured events:
- `pallet_tables::SchemaUpdated`
- `pallet_tables::TableDropped`
- `pallet_indexing::QuorumReached`

## Operational requirements

For the producer to actually persist payloads the host node must have
**offchain indexing enabled**. Producer call sites write via
`sp_io::offchain_index::set`, which is a no-op unless the host turns
offchain indexing on. Substrate defaults it to off; the embedding node
is responsible for exposing a way to enable it.

How it's enabled is a property of the node binary, not this pallet.
