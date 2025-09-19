## [1.28.2](https://github.com/spaceandtimefdn/sxt-node/compare/v1.28.1...v1.28.2) (2025-09-19)


### Bug Fixes

* disallow double submission and prioritize privileged quorum in `indexing::submit_data_inner` ([2946c00](https://github.com/spaceandtimefdn/sxt-node/commit/2946c00cc97c5ae56f49d14138f16faf86f6cc70))



## [1.28.1](https://github.com/spaceandtimefdn/sxt-node/compare/v1.28.0...v1.28.1) (2025-09-19)


### Bug Fixes

* fix pallet call index in attestation ([b830e6a](https://github.com/spaceandtimefdn/sxt-node/commit/b830e6a0182602f6794350945af48761025b9cc4))
* regenerate chain runtime ([ca01577](https://github.com/spaceandtimefdn/sxt-node/commit/ca01577f041a4353f4f2b3cd62444918f0ba8e8f))



# [1.28.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.27.0...v1.28.0) (2025-09-18)


### Features

* extend proof-of-sql scalars with MontScalarExt ([18af7af](https://github.com/spaceandtimefdn/sxt-node/commit/18af7afd3beb707ae128f4b567503e6eba8286c3))
* supertrait CommitmentId scalars with MontScalarExt ([0cc1d17](https://github.com/spaceandtimefdn/sxt-node/commit/0cc1d17445656de33cf0d32a313c949fe3211155))
* upgrade proof-of-sql to 0.121.1 ([fd5cac4](https://github.com/spaceandtimefdn/sxt-node/commit/fd5cac47e2e8b19c8c034ac6d2a440e8a64d9c40))
* use native blake3 conversion to scalar for varchar columns ([0253ae0](https://github.com/spaceandtimefdn/sxt-node/commit/0253ae06053454159f9d5ebd571fb191f9c8eacc))



# [1.27.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.26.4...v1.27.0) (2025-09-17)


### Bug Fixes

* bond_extra if staker is bonded, not if balance is nonzero ([3c35479](https://github.com/spaceandtimefdn/sxt-node/commit/3c35479d4d840d69f9fea1d153b1892927d6ced2))
* burn tokens that are withdrawn immediately ([8779acb](https://github.com/spaceandtimefdn/sxt-node/commit/8779acbb1c6b7e824799f12e704455b85d4cf3f2))


### Features

* increment runtime spec_version to 235 ([771da65](https://github.com/spaceandtimefdn/sxt-node/commit/771da65cd3965099b99de3dc8d236144d867d9fd))
* regenerate subxt file for runtime 235 ([6beb831](https://github.com/spaceandtimefdn/sxt-node/commit/6beb8316873f13ae57945878c77df154bbd026d5))
* track unstakes that have been claimed via system tables pallet ([0380d30](https://github.com/spaceandtimefdn/sxt-node/commit/0380d30ca9ab9757bbf97fc77073a8706f351a0c))



## [1.26.4](https://github.com/spaceandtimefdn/sxt-node/compare/v1.26.3...v1.26.4) (2025-09-17)


### Bug Fixes

* Fix Canary Logs ([ae29d9c](https://github.com/spaceandtimefdn/sxt-node/commit/ae29d9c5b8b356c7c892c088e76c23735d28cfbb))



