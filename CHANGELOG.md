# [1.35.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.34.1...v1.35.0) (2025-10-14)


### Features

* clamp unstaking claim amounts to be at least 1 ([18de587](https://github.com/spaceandtimefdn/sxt-node/commit/18de587562f1dfbf04f05cd40b9d724929e40d43))



## [1.34.1](https://github.com/spaceandtimefdn/sxt-node/compare/v1.34.0...v1.34.1) (2025-10-07)



# [1.34.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.33.1...v1.34.0) (2025-10-02)


### Features

* Add more functionality to Canary Service ([#92](https://github.com/spaceandtimefdn/sxt-node/issues/92)) ([c5d43a1](https://github.com/spaceandtimefdn/sxt-node/commit/c5d43a1a37c87d7495c9bbdd32be134990cc5022))



## [1.33.1](https://github.com/spaceandtimefdn/sxt-node/compare/v1.33.0...v1.33.1) (2025-09-29)


### Performance Improvements

* cpu-perf-enabled pallet-indexing weights to calculate fees ([0a6b184](https://github.com/spaceandtimefdn/sxt-node/commit/0a6b184c20f3766c6fb63449abc75d03256babcb))
* enable cpu-perf for all non-aarch64 targets ([66ad54d](https://github.com/spaceandtimefdn/sxt-node/commit/66ad54d86da9eb5be31dd763d78102e8064706e4))
* increment node version to 1.2.2 ([c7cccf7](https://github.com/spaceandtimefdn/sxt-node/commit/c7cccf7ca3fac3e610f9b21be40e9321fd7e9b08))
* increment runtime spec version to 240 ([4fd0378](https://github.com/spaceandtimefdn/sxt-node/commit/4fd0378147ef104231674f8aa5f22a998c2792ed))
* recalculate weights after re-enabling cpu-perf ([75acab0](https://github.com/spaceandtimefdn/sxt-node/commit/75acab0d540362567041a7ac321fc5cd7960707f))



# [1.33.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.32.7...v1.33.0) (2025-09-27)


### Bug Fixes

* make proof-of-sql-unchecked-deserialize no-std ([0c881e5](https://github.com/spaceandtimefdn/sxt-node/commit/0c881e559e5c31459217dd4fde94a738c6eb8133))
* make table_commitment_util fallible ([46f59ad](https://github.com/spaceandtimefdn/sxt-node/commit/46f59ad4a5290de9d9b83daae98fb7d27c5cd646))


### Features

* add `proof-of-sql-unchecked-deserialize` crate ([9badb1d](https://github.com/spaceandtimefdn/sxt-node/commit/9badb1df58174ee1d1c72734bf7d873f83b59026))


### Performance Improvements

* increment node version to 1.2.1 ([ffad438](https://github.com/spaceandtimefdn/sxt-node/commit/ffad438c8086d72ede50f8a1baa0ad939a4b1949))
* increment runtime spec version to 239 ([ff3b6ec](https://github.com/spaceandtimefdn/sxt-node/commit/ff3b6ec63c6e00dd355770beaee36cb117b33be1))
* recalculate weights after unchecked deserialization change ([ebb38b8](https://github.com/spaceandtimefdn/sxt-node/commit/ebb38b85c117e07b770b20bc07001812b2fa197e))
* use new pallet-indexing weights to calculate fees ([7dd3c1a](https://github.com/spaceandtimefdn/sxt-node/commit/7dd3c1a56464396c7d3ad2a5c3f6a63243ba64e9))
* use unchecked deserialize in canonical commitment bytes conversion ([140a740](https://github.com/spaceandtimefdn/sxt-node/commit/140a7404e066ad6ba6f13e5bc35a49302d5feecf))



