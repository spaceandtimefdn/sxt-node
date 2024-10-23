# [0.36.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.35.1...v0.36.0) (2024-10-23)


### Bug Fixes

* Added a check to ensure a schema exists for the table submitted by ([47da199](https://github.com/spaceandtimelabs/sxt-node/commit/47da199f69ab58f6f76e97a7ab227db54a67cbd2))
* Make sure events are emitted during genesis spec for table creation ([1696878](https://github.com/spaceandtimelabs/sxt-node/commit/169687854b59087e3782a1157279a0845d84fad8))
* Migrate flightsql integration to match server examples ([604df0c](https://github.com/spaceandtimelabs/sxt-node/commit/604df0cc3661a0d57522e21053b0bb32a64b9a89))


### Features

* Update snapshot and genesis to v2 ([ea739c3](https://github.com/spaceandtimelabs/sxt-node/commit/ea739c35a67d9f53ba18ec30f81bcb7ffd0bf9dd))
* Update subxt generated code for testnet runtime ([7c68796](https://github.com/spaceandtimelabs/sxt-node/commit/7c68796ed11c8367096f74c3b576483025d2f3b9))



## [0.35.1](https://github.com/spaceandtimelabs/sxt-node/compare/v0.35.0...v0.35.1) (2024-10-23)


### Bug Fixes

* use proof-of-sql 0.33 generated public parameters ([048e61c](https://github.com/spaceandtimelabs/sxt-node/commit/048e61c7198107efabb4068e4461210b258baa0d))



# [0.35.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.34.1...v0.35.0) (2024-10-23)


### Features

* process creation of empty tables in tables pallet ([5365ef1](https://github.com/spaceandtimelabs/sxt-node/commit/5365ef1c4c0b07c1f03ee7e9fd640388df3bc858))



## [0.34.1](https://github.com/spaceandtimelabs/sxt-node/compare/v0.34.0...v0.34.1) (2024-10-23)


### Bug Fixes

* allow timestamps without timezone in DDL ([592223c](https://github.com/spaceandtimelabs/sxt-node/commit/592223cef74552674179a820e7b1a86c08f3b0d4))
* map Int64 columns to proof-of-sql BigInt ([309a85c](https://github.com/spaceandtimelabs/sxt-node/commit/309a85cd7ce853ef77b3f353693d54d3028e13a9))
* preserve record batch timezone existence in on-chain-table ([90122de](https://github.com/spaceandtimelabs/sxt-node/commit/90122de511e40d3251027d77fe09f8e9a4821285))



# [0.34.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.33.0...v0.34.0) (2024-10-23)


### Bug Fixes

* force proof-of-sql and postgres compliance on insert data ([58e64f9](https://github.com/spaceandtimelabs/sxt-node/commit/58e64f9c4c02a692cf5e608f88045d3245cf77cf))
* force proof-of-sql compliance on table definitions ([fe54fbc](https://github.com/spaceandtimelabs/sxt-node/commit/fe54fbce38e6eaf60cd440ee186475f92fe615f8))


### Features

* make non-arrow data-compliance functions no_std compatible ([2adf5c3](https://github.com/spaceandtimelabs/sxt-node/commit/2adf5c3afa7471ad3f74b03c4b8aab145339dad1))



