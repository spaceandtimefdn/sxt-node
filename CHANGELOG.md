# [1.70.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.69.1...v1.70.0) (2026-10-29)


### Bug Fixes

* default prover-db indexer to disabled unless explicitly enabled ([360803d](https://github.com/spaceandtimefdn/sxt-node/commit/360803dbafa42b2501fd7c1c4c99d450ab778414))
* fallback to client hash when `BlockHash` is missing ([95794da](https://github.com/spaceandtimefdn/sxt-node/commit/95794da45e258bbcb5cdb6edb9cea1b8b4997efc))
* make on chain indexer keys saturate rather than default to `0` ([5f794de](https://github.com/spaceandtimefdn/sxt-node/commit/5f794de76f0bb67b632c64e39f0a6f4422a810e1))
* measure pallet-indexing weights for runtime 250 ([8c30d41](https://github.com/spaceandtimefdn/sxt-node/commit/8c30d41b6e7c1b27d6e5950fa3f353c672f76b1f)), closes [#219](https://github.com/spaceandtimefdn/sxt-node/issues/219)
* rollback to git commit hash 6e668a97b5ffad477aa44318475ab381c4312524 ([3d9b9d4](https://github.com/spaceandtimefdn/sxt-node/commit/3d9b9d462a23d839d2150cd3094ce094fbbe0953))
* use finalized block in prover-db-indexer ([50438ab](https://github.com/spaceandtimefdn/sxt-node/commit/50438abdc58364ae4cae18119d3db1582a88fd51))


### Features

* add `db_events_at` helper that queries the client for `DBEvent`s. ([4fc3952](https://github.com/spaceandtimefdn/sxt-node/commit/4fc39528ab8ff6480c4b6be724372a866afd023b))
* add `REFUND_PERCENTAGE_DOMAIN` table metadata domain ([21c396c](https://github.com/spaceandtimefdn/sxt-node/commit/21c396c29919b70c28f2a2bcbed03cecb2c7a741))
* add `refund_quorum` for refunding quorum submitters ([a39eb60](https://github.com/spaceandtimefdn/sxt-node/commit/a39eb60c168fcfdb4724010f4387a8b2bdeac561))
* add `SubmitDataCall` type ([ae14a7d](https://github.com/spaceandtimefdn/sxt-node/commit/ae14a7ddd152c69fd465c16f93e006745e465f2a))
* add `table_treasury_account` helper ([7c99cf6](https://github.com/spaceandtimefdn/sxt-node/commit/7c99cf6c7b169add4d90665727c2ddbc9ce1775a))
* add client native interface and extenality ([d9c6c4d](https://github.com/spaceandtimefdn/sxt-node/commit/d9c6c4d6dbd4deb6eb6d50aa8206bf4f282e44fe))
* add generic config key-value store for offchain workers ([117b845](https://github.com/spaceandtimefdn/sxt-node/commit/117b845c7e1c028ba7705e261c263d15fda84f28))
* add pallet-parameters with root-settable per-diem reward rate ([b45ad6d](https://github.com/spaceandtimefdn/sxt-node/commit/b45ad6d55588fcae8ad8d40338553adf2a69923b))
* adds `client::hash` ([a5b581d](https://github.com/spaceandtimefdn/sxt-node/commit/a5b581d0c3eae8e2f6dedc7767ff8757e4a88e06))
* collect transaction fees in pallet-rewards ([262a625](https://github.com/spaceandtimefdn/sxt-node/commit/262a6257f59a34bac651833320db60ee7937a077))
* Document Snapshot Download Process ([4c2608b](https://github.com/spaceandtimefdn/sxt-node/commit/4c2608b1a55b989b261c35a20f84472d20387598))
* drop `EventCapture` from prover_db_indexer ([b3381c5](https://github.com/spaceandtimefdn/sxt-node/commit/b3381c56c1a7a26594f71ec509f38a3a0b01f3e4))
* make transaction fees root-settable ([af95e40](https://github.com/spaceandtimefdn/sxt-node/commit/af95e40d30950ced96a65cd7c4a47405c23166f9))
* pay validators a per-diem rate of total stake plus collected fees ([6aa6496](https://github.com/spaceandtimefdn/sxt-node/commit/6aa6496262def149639f9808b10a80b30e9a14e3))
* refund submission fee to quorum submitters ([47c7eaa](https://github.com/spaceandtimefdn/sxt-node/commit/47c7eaa927ee89f0c05ad8a863f5dca76998dee1))
* register client externality to offchain worker ([338c213](https://github.com/spaceandtimefdn/sxt-node/commit/338c2134941e196551915ef620152d5a5bd4d6b4))
* use client for `prover_db_indexer` ([6679c06](https://github.com/spaceandtimefdn/sxt-node/commit/6679c06303b13bdb538b225352e9411e4741a92e))
* use generic key value config for prover_db_indexing instead of storage/CLI based config ([faff564](https://github.com/spaceandtimefdn/sxt-node/commit/faff564feef8fff2f72848718c13a9a5b79771d9))



## [1.69.1](https://github.com/spaceandtimefdn/sxt-node/compare/v1.69.0...v1.69.1) (2026-06-25)


### Bug Fixes

* update weight constant for fee calculation ([5fc710a](https://github.com/spaceandtimefdn/sxt-node/commit/5fc710a77839104fd1d61d33ed3d8f7e122c27f3))



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



