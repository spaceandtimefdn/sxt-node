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



# [0.33.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.32.0...v0.33.0) (2024-10-23)


### Features

* upgrade proof-of-sql to version 0.33 ([e0cd63e](https://github.com/spaceandtimelabs/sxt-node/commit/e0cd63e7d0bd778a3f93288534e15622e8b37e70))



# [0.32.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.31.1...v0.32.0) (2024-10-23)


### Features

* add empty data-compliance-please-deprecate-me crate ([a6d3962](https://github.com/spaceandtimelabs/sxt-node/commit/a6d3962cd89dd3333861bb682d2b28c3928b61e0))
* add record batch mapping utilities ([f2c45dc](https://github.com/spaceandtimelabs/sxt-node/commit/f2c45dcbe85ebbf05e708d9fda148a543c9e2e7f))
* clamp decimal precision to proof-of-sql maximum ([cbffb7b](https://github.com/spaceandtimelabs/sxt-node/commit/cbffb7bc0ab16442bac0e443b52ac5caca276f23))
* parse some utf8 columns to decimal ([31ac14d](https://github.com/spaceandtimelabs/sxt-node/commit/31ac14dc473a7c3d661a61fc68746f47e0a34b6f))
* remove null bytes from string columns ([c8e5720](https://github.com/spaceandtimelabs/sxt-node/commit/c8e5720f1528f5bb876f24261bf57e8064693b41))
* replace nulls with defaults ([333b3cd](https://github.com/spaceandtimelabs/sxt-node/commit/333b3cd89efac78ee70e58f34735774a3304bb57))



## [0.31.1](https://github.com/spaceandtimelabs/sxt-node/compare/v0.31.0...v0.31.1) (2024-10-21)


### Bug Fixes

* Added credentials for FlightSQL Client and panic on any errors in ([87f5bde](https://github.com/spaceandtimelabs/sxt-node/commit/87f5bdecd925cadd78270c0ed06f691fa895722b))



