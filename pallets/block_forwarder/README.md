# pallet-block-forwarder

Offchain worker that forwards table lifecycle and data events to an
external HTTP indexer service using protobuf-over-HTTP.

Watches for these events from other pallets and relays them:
- `pallet_tables::SchemaUpdated`
- `pallet_tables::TablesCreatedWithCommitments`
- `pallet_tables::TableDropped`
- `pallet_indexing::QuorumReached`

Wire contract is under `proto/indexer.proto`; five POST endpoints at
`/v1/{create_table,drop_table,put_batches,checkpoint,get_last_checkpoint}`.

## Operational requirements

The pallet has **two** node-side flags that must be correctly set for
forwarding to work. Getting either wrong produces a silent no-op on the
producer side — events look like they'd be forwarded but nothing arrives
at the indexer.

### 1. `--enable-offchain-indexing=true` — **required**

The producer (`on_finalize` hook) writes events into offchain local
storage via `sp_io::offchain_index::set`. That host function is a
**silent no-op unless `--enable-offchain-indexing=true` is passed on the
command line**. Substrate defaults the flag to `false`.

Symptom if missing: `on_finalize(N): writing K events to offchain DB`
logs appear normally, but the OCW's `offchain_index::read(N)` returns
`None` and no forwarding happens.

As of commit `dd873ed`, the node boots with a loud `warning:` to stderr
if this flag is not set; running with `--indexer-url` *without* the flag
is a hard startup error.

### 2. `--offchain-worker=always` — required on dev nodes

Substrate defaults offchain-worker scheduling to `when-authority` —
OCWs only fire on validator/collator nodes. For a dev `--dev --tmp`
node, that means zero OCW invocations. Pass `--offchain-worker=always`
to force them to run regardless of role.

### 3. `--indexer-url <URL>` — optional (but usually what you want)

Tells the OCW where to POST forwarded events. Mirrors what
`scripts/configure-ocw.sh` does via the `offchain_localStorageSet` RPC,
but:
- Applies before the first block, so no race with early events.
- Doesn't require `--rpc-methods=unsafe` (the RPC is unsafe-gated).
- Doesn't need a second process / second terminal.

If omitted, the OCW stays dormant until the URL is written some other
way (RPC, a persistent base-path node with the URL already set from a
previous run, etc.).

## Minimal one-command dev setup

```
./target/release/sxt-node \
  --dev --tmp \
  --rpc-cors=all \
  --offchain-worker=always \
  --enable-offchain-indexing=true \
  --indexer-url http://127.0.0.1:9999
```

`--rpc-methods=unsafe` is **not** needed any more for indexer setup —
only include it if you still want the old `configure-ocw.sh` RPC path.

## Runtime wiring

Three Config associated types; the runtime provides them:

```rust
impl pallet_block_forwarder::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    // Used to resolve pallet_indexing's pallet index dynamically via
    // PalletInfoAccess::index(), so reordering construct_runtime! never
    // breaks the filter silently.
    type IndexingPallet = Indexing;

    // Variant index of `QuorumReached` within pallet_indexing::Event.
    // The runtime supplies a resolver that looks the variant up by name
    // at startup via scale_info::TypeInfo. See
    // `DynamicQuorumReachedIndex` in runtime/src/lib.rs.
    type QuorumReachedVariantIndex = DynamicQuorumReachedIndex;
}
```

## Data format

- `CreateTable.arrow_schema`: raw `CREATE TABLE …` DDL bytes (UTF-8).
  The HTTP server parses them via `sqlparser` to derive the Arrow schema.
- `PutBatches.record_batch`: Arrow IPC single-batch stream bytes,
  identical to what `pallet_indexing::submit_data.data` validates and
  what `QuorumReached` relays verbatim. The forwarder never decodes
  this payload — it's an opaque pass-through.

The pallet does **not** introduce any new host functions; everything
goes through already-in-stable `sp_io` APIs (`offchain_index::set`,
`offchain::http::Request`, etc.).

## Dedup key contract

Every forwarded table is registered with the dedup key column
`META_ROW_NUMBER`. The HTTP server rejects `CreateTable` requests whose
Arrow schema lacks a `META_ROW_NUMBER BIGINT NOT NULL` column. If your
on-chain table DDLs don't include this column, the first forward
attempt will fail at the server. Either add the column to the DDL or
patch the forwarder's `DEDUP_KEY_COLUMN` constant.

## Testing

- `cargo test -p pallet-block-forwarder` — 7 unit/integration tests
  covering skip/forward/delete/multi-block/resume.
- `mock-server/` (separate workspace member) — axum-based local HTTP
  stub that logs each call; point `--indexer-url` at it for quick local
  iteration without a real indexer.
- `scripts/run-local-demo.sh` — tmux orchestrator that brings up the
  mock server + node + OCW configure in one go. Pre-`--indexer-url`
  workflow; still works if you want to test the RPC path.
