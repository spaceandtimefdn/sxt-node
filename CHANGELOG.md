# [1.69.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.68.0...v1.69.0) (2026-06-22)


### Bug Fixes

* add basic documentation to runtime crate ([30c26e7](https://github.com/spaceandtimefdn/sxt-node/commit/30c26e7fbef1c919c08995dd8e388b9fb0a0526f))
* add missing docs for runtime module items ([52a2905](https://github.com/spaceandtimefdn/sxt-node/commit/52a2905d2b79deb117f212b25c9d1532875c8659))
* add missing docs to chain-utils ([c8235c6](https://github.com/spaceandtimefdn/sxt-node/commit/c8235c60213336ad25d645cac7a055c30c08cdc0))
* address warnings in on-chain-table ([095a856](https://github.com/spaceandtimefdn/sxt-node/commit/095a856ca25da625fefbee42087d82f7e857b069))
* address warnings in pallet-rewards mock module ([886bc36](https://github.com/spaceandtimefdn/sxt-node/commit/886bc362d6ae7d4f302c33a9f5aac6b73ac0e562))
* allow deprecated CurrencyAdapter usage ([cfe0517](https://github.com/spaceandtimefdn/sxt-node/commit/cfe05170ee3f1e43ba3986e1eb93dc7127772ef5))
* allow missing docs in private items in test_end_row_limits binary ([7700cd6](https://github.com/spaceandtimefdn/sxt-node/commit/7700cd626c0b2427e750a9aa258cbd3331a233b3))
* allow missing_docs_in_private_items on event forwarder contract ([b2d6f8b](https://github.com/spaceandtimefdn/sxt-node/commit/b2d6f8b632e46b924ca334de8e8d6a7abd8b2b9b))
* allow unused private_key variables in pallet-keystore benchmarks ([c218f0d](https://github.com/spaceandtimefdn/sxt-node/commit/c218f0d619ec00a2a9146c1732815e7e52f0f32f))
* declare anonymous lifetime in commitment-sql proptest helper ([63b3fc0](https://github.com/spaceandtimefdn/sxt-node/commit/63b3fc04446c1c778a626eade6b78c1c03d56430))
* declare anonymous lifetimes in canaries parse functions ([f81e9de](https://github.com/spaceandtimefdn/sxt-node/commit/f81e9dee38bf76d2bf29bc94a046568ed5b67ee3))
* document all items in watcher main module ([2fe60db](https://github.com/spaceandtimefdn/sxt-node/commit/2fe60dbce90d8c9aa6479c4ffb3260ba39f7d47b))
* document memory_commitment_map module in commitment-map ([32a38e3](https://github.com/spaceandtimefdn/sxt-node/commit/32a38e3454a2fe14e70cb60d7bead75678206b38))
* document missing items in canaries ([72c5f33](https://github.com/spaceandtimefdn/sxt-node/commit/72c5f331dbe19c8329b483b94b8892c0071a6ab6))
* document missing items in event-forwarder ([5408075](https://github.com/spaceandtimefdn/sxt-node/commit/54080750e0a18bcbd6e17c7d3a999455b3c73ccd))
* document modules in chain-utils ([3eb6545](https://github.com/spaceandtimefdn/sxt-node/commit/3eb654592de728f0ad97a2b03e273fc7b957e12c))
* document modules in rpc crate ([19cf1af](https://github.com/spaceandtimefdn/sxt-node/commit/19cf1afb35ca8a6094d04d13ba783642fb29b41a))
* don't hide elided lifetimes in backwards compatibility test case generator ([4e6968e](https://github.com/spaceandtimefdn/sxt-node/commit/4e6968e495782439a9b54c0dcba4bceb2b6c04a5))
* don't import frame_benchmarking if runtime-benchmarks is disabled in node ([32cfca3](https://github.com/spaceandtimefdn/sxt-node/commit/32cfca3c77bc4438a41d795491f72583a3342793))
* expect AttestationInfo::block_number to be dead code in canaries ([7753c44](https://github.com/spaceandtimefdn/sxt-node/commit/7753c44192d2201c03c3908fd017b4833925073d))
* expect some dead code in NewFullBase in node ([fe06f5e](https://github.com/spaceandtimefdn/sxt-node/commit/fe06f5e634855ea344f8a873e64ab2e5ac4011ed))
* ignore unused error variable in watcher main module ([d330fe5](https://github.com/spaceandtimefdn/sxt-node/commit/d330fe560be31925960ac61a13e82e5fdf5c8a42))
* ignore unused variable in attestation-tree test ([ce8adf3](https://github.com/spaceandtimefdn/sxt-node/commit/ce8adf382f3f23a2fbf41fb7bccf793a14e826f7))
* ignore unused variable in pallet-attestation benchmarks ([b0ef4a7](https://github.com/spaceandtimefdn/sxt-node/commit/b0ef4a7fa455149bfeb1f7df0dc3d10a49b6f2e6))
* implement benchmark configurations for runtime outside runtime api implementation ([d1a3db8](https://github.com/spaceandtimefdn/sxt-node/commit/d1a3db89464435e4cf40a8973fb168b7a69f9dc4))
* only define migrations with runtime-benchmarks disabled ([26e8804](https://github.com/spaceandtimefdn/sxt-node/commit/26e88049c6475f2b7d89f7b35f9725a37ecf036d))
* privatize deposit_event in pallet-rewards ([6c1caaf](https://github.com/spaceandtimefdn/sxt-node/commit/6c1caaf7883733038f12ca6865d35ef8ecd7390a))
* privatize deposit_event in pallet-smartcontracts ([fe43a19](https://github.com/spaceandtimefdn/sxt-node/commit/fe43a196cef7e8f4c6d58f41068a39257f71da66))
* privatize deposit_event in pallet-system-tables ([8bf7c21](https://github.com/spaceandtimefdn/sxt-node/commit/8bf7c211f71f2767315d3251699456286b2d40bf))
* privatize deposit_event in pallet-tables ([da951a5](https://github.com/spaceandtimefdn/sxt-node/commit/da951a5e1d2f266d23e1be41e6d3a5204d8ddc33))
* privatize pallet-keystore deposit_event ([e878e37](https://github.com/spaceandtimefdn/sxt-node/commit/e878e37f27e985985ebf3bd5f349cfabe9030a94))
* privatize pallet-permissions deposit_event ([099a7ba](https://github.com/spaceandtimefdn/sxt-node/commit/099a7bac092be42b62ca124655f9cafe1c3e4803))
* privatize system-contracts deposit_event ([305dfc9](https://github.com/spaceandtimefdn/sxt-node/commit/305dfc9a654d2612b392b5ad04e7b97eabe816c5))
* privatize zkpay deposit_event ([9d5993c](https://github.com/spaceandtimefdn/sxt-node/commit/9d5993ce4b6838b0949d1610bf7508eb0e9d7fbd))
* propagate tui error in update_ui in watcher ([fe943eb](https://github.com/spaceandtimefdn/sxt-node/commit/fe943eb344d050e68fe357e1f54374587dec1cdf))
* remove dead code in chain-utils ([0070600](https://github.com/spaceandtimefdn/sxt-node/commit/007060076dae28110dcaa31233871ecc9e7b2c57))
* remove dead code in node ([6958ec6](https://github.com/spaceandtimefdn/sxt-node/commit/6958ec6dc287ed82417183959ce9d40bcbe73c76))
* remove unused block_number parameter from verify_attestations in watcher ([6225426](https://github.com/spaceandtimefdn/sxt-node/commit/6225426f01e6d700a8ce5fde7f4d46504563fd19))
* remove unused block_number parameter from verify_signature in watcher ([08f3bff](https://github.com/spaceandtimefdn/sxt-node/commit/08f3bff29026f3b778f4db32523309e99506b0d6))
* remove unused const in pallet-tables test ([e7bf9f5](https://github.com/spaceandtimefdn/sxt-node/commit/e7bf9f5a4489ca15200c170381171e8e0498ab0a))
* remove unused const in runtime test ([c7e0add](https://github.com/spaceandtimefdn/sxt-node/commit/c7e0add0f29fe0e9bfed6876675480a80195eba5))
* remove unused dependencies from sxt-core ([361f8a0](https://github.com/spaceandtimefdn/sxt-node/commit/361f8a068cc8af324010f1d3bb92ab2c97caee2f))
* remove unused destructure variables in verify_attestation in watcher ([dc04d24](https://github.com/spaceandtimefdn/sxt-node/commit/dc04d24c3796fd8c8f438386d37642419481bd29))
* remove unused import in memory_commitment_map test ([4c2300a](https://github.com/spaceandtimefdn/sxt-node/commit/4c2300af31f7fd6acb944e8b4b3fe4a43613b6ec))
* remove unused import in pallet-attestation benchmarks ([5fa662e](https://github.com/spaceandtimefdn/sxt-node/commit/5fa662eab769e0e5db6500b9a966f8d3d74181c9))
* remove unused import in pallet-commitments migrations ([ecf6a05](https://github.com/spaceandtimefdn/sxt-node/commit/ecf6a054c845e8d46c6fc6d1b4d6d59f054eb317))
* remove unused imports and format imports in chain-utils ([5cd269b](https://github.com/spaceandtimefdn/sxt-node/commit/5cd269b46ade733d498b2744ad61f1657f5cdb63))
* remove unused imports in attestation-tree ([91f5d61](https://github.com/spaceandtimefdn/sxt-node/commit/91f5d61be876ea6baaecfa79f96b61e028a528f9))
* remove unused imports in node ([1f4c7a8](https://github.com/spaceandtimefdn/sxt-node/commit/1f4c7a8cf4fc0ef973b582a6601d02eddfbf2918))
* remove unused imports in pallet-smartcontracts ([b7fa723](https://github.com/spaceandtimefdn/sxt-node/commit/b7fa7236c8e461bf18b1474cb5d388c8c0b32563))
* remove unused imports in pallet-tables ([a08ccfd](https://github.com/spaceandtimefdn/sxt-node/commit/a08ccfd263410d96dddeba53a4b7b13bd95293d0))
* remove unused imports in runtime ([614a1e3](https://github.com/spaceandtimefdn/sxt-node/commit/614a1e39c35c92d1f35c6be6c6c862f6e1b0427e))
* remove unused imports in test_end_row_limits ([0328143](https://github.com/spaceandtimefdn/sxt-node/commit/0328143ffee93399faed6c9f21ec9f5b093f92fb))
* remove unused imports in watcher ([340d27b](https://github.com/spaceandtimefdn/sxt-node/commit/340d27bb51a8df5d91950920b9d1bcced8e936b3))
* remove unused substrate key path from watcher client ([2bd4d02](https://github.com/spaceandtimefdn/sxt-node/commit/2bd4d0292b70833629f812adafdfaefd87b0d142))
* switch hex to being a dev dependency in rpc crate ([7b01208](https://github.com/spaceandtimefdn/sxt-node/commit/7b01208b1b8d8d747147f877b3826f53e5509bf8))
* warn unused_crate_dependencies in sxt-core ([62c85b7](https://github.com/spaceandtimefdn/sxt-node/commit/62c85b7f243a49f5714251b75187b07376c88486))


### Features

* add configurable include set to gate captured tables ([#193](https://github.com/spaceandtimefdn/sxt-node/issues/193)) ([ab1cf7f](https://github.com/spaceandtimefdn/sxt-node/commit/ab1cf7f63d257bd8e8f42ad7dd6a6f0a21c9a335))
* add http client for prover db indexer ([#190](https://github.com/spaceandtimefdn/sxt-node/issues/190)) ([617f4ef](https://github.com/spaceandtimefdn/sxt-node/commit/617f4ef357ef73a706c27d205a90159dcaa72543))
* Add OCW consumer that drains and forwards events ([#192](https://github.com/spaceandtimefdn/sxt-node/issues/192)) ([ec1d24c](https://github.com/spaceandtimefdn/sxt-node/commit/ec1d24c284ff759f611ea11315b41cffa9420745)), closes [#187](https://github.com/spaceandtimefdn/sxt-node/issues/187) [#190](https://github.com/spaceandtimefdn/sxt-node/issues/190)
* allow SCI namespace creation without special permissions ([092ba68](https://github.com/spaceandtimefdn/sxt-node/commit/092ba6844f760b5692cc092c6e83a894852de044))
* configure prover_db_url from CLI and into local storage ([#186](https://github.com/spaceandtimefdn/sxt-node/issues/186)) ([066b719](https://github.com/spaceandtimefdn/sxt-node/commit/066b719ee74539c7403851548b1aaaa5c5afcf7c))
* prover db indexer producer ([#187](https://github.com/spaceandtimefdn/sxt-node/issues/187)) ([dbd13dd](https://github.com/spaceandtimefdn/sxt-node/commit/dbd13dd2447eb2c8c06a5aeb1e5e5ec8d1e88168))



# [1.68.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.67.0...v1.68.0) (2026-03-24)


### Features

* bump `spec_version` to 248 ([ca451f0](https://github.com/spaceandtimefdn/sxt-node/commit/ca451f0570283048771b8c4819a98758ca36492e))



# [1.67.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.66.0...v1.67.0) (2026-03-23)


### Features

* add ensure_root_or_any_permissioned and tests ([b21c079](https://github.com/spaceandtimefdn/sxt-node/commit/b21c07992d1c5e8b73100c1da047fafa5ed033fc))
* add has_any_permissions and tests ([76e038a](https://github.com/spaceandtimefdn/sxt-node/commit/76e038a59f901d35041493164535e954fa278e3d))
* use ensure_root_or_any_permissioned in update_table_quorum to allow UpdateTableQuorum permission ([f17e23f](https://github.com/spaceandtimefdn/sxt-node/commit/f17e23fbc5a417a5e301bd628537067c65f6da4f))



# [1.66.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.65.0...v1.66.0) (2026-03-23)


### Features

* allow SCI table creation without special permissions ([c9631de](https://github.com/spaceandtimefdn/sxt-node/commit/c9631de06084396aa92e370fbe74895f5c7d4e0e))



# [1.65.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.64.3...v1.65.0) (2026-03-19)


### Bug Fixes

* remeasure pallet-indexing weights with n parameter ([4dde534](https://github.com/spaceandtimefdn/sxt-node/commit/4dde5340fbc10fc01275e296f6356b7963247dfb))


### Features

* base benchmarks on num_cols and total number of elements ([e3304a1](https://github.com/spaceandtimefdn/sxt-node/commit/e3304a1eb171d560ffe357a2375a9fd465efd762))
* update spec version to 247 ([856d7d2](https://github.com/spaceandtimefdn/sxt-node/commit/856d7d23414b24484e971a7d47524f1f189caa1d))



