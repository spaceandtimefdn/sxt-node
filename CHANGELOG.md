# [0.7.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.6.0...v0.7.0) (2024-10-02)


### Features

* add OnChainColumn::empty_with_type method ([0e0ffe1](https://github.com/spaceandtimelabs/sxt-node/commit/0e0ffe1860ac7513549bec3fc075d4c95c7e6da5))
* add OnChainColumn::try_to_committable_column method ([e3ebd4a](https://github.com/spaceandtimelabs/sxt-node/commit/e3ebd4ac6f5532898fcfbad690290e88a5b05ea3))
* add OnChainTable::iter_committable method ([519eedd](https://github.com/spaceandtimelabs/sxt-node/commit/519eedde2ed6650338902c55d83c212721b60b6a))
* implement conversion from U256 to scalar ([cd9577c](https://github.com/spaceandtimelabs/sxt-node/commit/cd9577c9e22dea58e2c42808b033c0beb8ffb31c))



# [0.6.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.5.0...v0.6.0) (2024-10-01)


### Features

* Copy the template pallet folder to create the skeleton for the indexing ([9bb907a](https://github.com/spaceandtimelabs/sxt-node/commit/9bb907a7cab93a5612dee8e2966c7e31f120cece))
* Implement Indexing pallet with basic data submissions and quorum ([e9322db](https://github.com/spaceandtimelabs/sxt-node/commit/e9322db337d48f0076015f95a9621ba2adcbbcc5))
* Integrate the indexing pallet into the runtime ([41798b8](https://github.com/spaceandtimelabs/sxt-node/commit/41798b8221f83948b1fb882dbd1e5146172f6d59))



# [0.5.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.4.0...v0.5.0) (2024-10-01)


### Bug Fixes

* typo ([f92933a](https://github.com/spaceandtimelabs/sxt-node/commit/f92933ac28875cd0628a5506184f207a2074eb9d))


### Features

* adding ratatelogs from apache2-utils to handle pg logs, also change the way script works to act like simple init process to trap system signals after initial process spwan ([bc4d2ee](https://github.com/spaceandtimelabs/sxt-node/commit/bc4d2ee629c6fa53c95487bd4374b4faf8b340ef))



# [0.4.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.3.0...v0.4.0) (2024-09-30)


### Bug Fixes

* disable default-features for primitive-types ([ff93198](https://github.com/spaceandtimelabs/sxt-node/commit/ff93198a0ef11c6d79d85e4661a8853cce94fb35))
* hash indexmap with ahash in on-chain-table ([6f44839](https://github.com/spaceandtimelabs/sxt-node/commit/6f44839063bf37734fab3f794d84f50ce27bb55a))



# [0.3.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.2.4...v0.3.0) (2024-09-27)


### Features

* add on-chain-table crate ([314c929](https://github.com/spaceandtimelabs/sxt-node/commit/314c929ab416a30a9f5441dfcf8b3b73e0ecb446))
* add OnChainColumn as no_std insert column type ([91623c6](https://github.com/spaceandtimelabs/sxt-node/commit/91623c61329d479664ea73b00d9c759eaf876d53))
* add OnChainTable type for no_std insert data ([d658551](https://github.com/spaceandtimelabs/sxt-node/commit/d6585515db44f371779e1530e69744d32e3a1d5d))
* add U256 and arrow i256 conversion utilities ([c788a0b](https://github.com/spaceandtimelabs/sxt-node/commit/c788a0baa134c6cd1188f668b17c5d0545765ca8))



