# `on-chain-table`
This crate provides the `OnChainTable` type for representing insert data.
This type is intended to..
- be used in `no_std` environments, like the `sxt-node` runtime
- support usage `proof-of-sql` commitment APIs.

In the future, `sxt-node` may switch to `arrow`'s `RecordBatch`.
`RecordBatch` is currently not `no_std` compatible.
In the meantime, `sxt-node` will use `RecordBatch`s in external APIs, but convert them to/from `OnChainTable` for runtime usage.
This crate provides these conversions under the `arrow` feature.
