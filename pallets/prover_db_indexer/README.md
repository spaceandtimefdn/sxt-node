# pallet-prover-db-indexer

Bridges in-runtime table lifecycle and data events to an external
prover-db indexer (HTTP, protobuf wire format).

The pallet has two halves:
- **Producer (in-runtime, synchronous).** `pallet-tables` and
  `pallet-indexing` call into this pallet via the `EventCapture` trait
  at extrinsic time, immediately after depositing the relevant event.
  The producer's cost is paid by the calling extrinsic's declared
  weight, not by an `on_finalize` hook.
- **Consumer (offchain worker).** An OCW drains the captured payloads
  from the offchain DB and POSTs them to the indexer's HTTP endpoints.

Captured events:
- `pallet_tables::SchemaUpdated`
- `pallet_tables::TableDropped`
- `pallet_indexing::QuorumReached`

Wire contract is under `proto/prover-db.proto`; five POST endpoints at
`/v1/{create_table,drop_table,put_batches,checkpoint,get_last_checkpoint}`.

## Operational requirements

For end-to-end forwarding the host node must have all three of the
following enabled. Any one missing is a silent no-op on the producer or
OCW side — events never reach the indexer.

1. **Offchain indexing enabled.** Producer call sites write events via
   `sp_io::offchain_index::set`, which is a no-op unless the host has
   offchain indexing turned on. Substrate defaults this to off; the
   embedding node is responsible for exposing a way to enable it.
2. **Offchain workers scheduled to run.** Substrate's default
   scheduling fires OCWs only on validator/collator roles. Non-authority
   nodes (dev, RPC-only) need OCW scheduling forced on for forwarding to
   happen.
3. **Indexer URL set in OCW persistent local storage.** The OCW reads
   the URL at the key exposed by `sxt_core::prover_db_indexer::PROVER_DB_URL_KEY`.
   The embedding node typically writes this once at startup; alternatively
   it can be set out-of-band (e.g. via `offchain_localStorageSet`).

How each is enabled is a property of the node binary, not this pallet.
