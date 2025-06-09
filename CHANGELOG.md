## [1.7.1](https://github.com/spaceandtimefdn/sxt-node/compare/v1.7.0...v1.7.1) (2025-06-09)



# [1.7.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.6.0...v1.7.0) (2025-06-06)


### Bug Fixes

* slim down watcher image from 1.44GB to ~100MB ([d365f17](https://github.com/spaceandtimefdn/sxt-node/commit/d365f1741d314c143902f5ba65568d781c380ee5))


### Features

* create /key as volume for attestor ([65536c0](https://github.com/spaceandtimefdn/sxt-node/commit/65536c017ad8074a65698a772f4591c3829fb530))
* include subkey in watcher image ([db4e501](https://github.com/spaceandtimefdn/sxt-node/commit/db4e50102116856fcc74b1c4b3006a74c0dda5e1))



# [1.6.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.5.0...v1.6.0) (2025-06-05)


### Bug Fixes

* add clarification on how to get the SS58 key for nomination ([5fed945](https://github.com/spaceandtimefdn/sxt-node/commit/5fed945b976a15d8b2768bfe836382cef9bb0dd7))
* add clarification on optional steps for docker vs k8s ([373a50a](https://github.com/spaceandtimefdn/sxt-node/commit/373a50a50cea7a4d41169384f373bb2d1cc511fa))
* add corrections to command flags passed to docker ([c64d6a4](https://github.com/spaceandtimefdn/sxt-node/commit/c64d6a48e0198b9f9f1c8d544172fca4d3e89326))
* add explicit instructions about exporting SECRET_SEED ([d869e63](https://github.com/spaceandtimefdn/sxt-node/commit/d869e631b97cb52bd1e7fd7a8706786cb53dba17))
* add steps to install rustfmt and clippy in macos CI. Changes to the apple rust ecosystem caused previously included components to be removed. These changes are gated to only occur for macos ([4ed747f](https://github.com/spaceandtimefdn/sxt-node/commit/4ed747f5d68b577c7fb09f9c5c29751701effe66))
* add telemetry-url flag to documentation ([9331deb](https://github.com/spaceandtimefdn/sxt-node/commit/9331debdf51874d2eb3ca193fa098e6dafcc5d9a))
* add workaround to file permissions issue in validator key volume ([28d78e2](https://github.com/spaceandtimefdn/sxt-node/commit/28d78e2ed1486d4c23a6764dcf98185dc0f3a024))
* address issue with copy-data instructions ([8e306a9](https://github.com/spaceandtimefdn/sxt-node/commit/8e306a946d7d7a6d6d83c6cd428ed2d5b9c932c7))
* format discord link as url like in other places ([2a799c7](https://github.com/spaceandtimefdn/sxt-node/commit/2a799c7588adddc48f25a73aa5ed52e18faf6212))
* include arguments used in chart v0.10.4 ([5e9bc31](https://github.com/spaceandtimefdn/sxt-node/commit/5e9bc3117b043247a00ed4a2c9cf1eec7f4e89cc))
* make SXT Discord a link ([524cf2c](https://github.com/spaceandtimefdn/sxt-node/commit/524cf2c41c0b222be695e9d7d23ecdddf3194a5e))
* remove references to helm ([b63d1ab](https://github.com/spaceandtimefdn/sxt-node/commit/b63d1ab0ada74f2ddcd800f8f9d2a9a88d4a83e9))
* remove unnecessary rpc flags ([eebd18f](https://github.com/spaceandtimefdn/sxt-node/commit/eebd18fac32bac4d3cb01867c763d77ec54c1c47))
* set sxt-testnet-data as external volume as well ([36a6b67](https://github.com/spaceandtimefdn/sxt-node/commit/36a6b67e372595a04868c1affee9ed3a0d59e41b))
* set the setup directory to writable location ([a7c14b1](https://github.com/spaceandtimefdn/sxt-node/commit/a7c14b1f17605ca4b68855e76ffbd85d46bcf689))
* syntax error ([#12](https://github.com/spaceandtimefdn/sxt-node/issues/12)) ([b9da7b1](https://github.com/spaceandtimefdn/sxt-node/commit/b9da7b18ac0343717e537f9ab294f6699b047bee))
* update Azure cloud SKU to match new system requirements ([13f83b0](https://github.com/spaceandtimefdn/sxt-node/commit/13f83b0616d6069cbe736a9b50c6b59716232065))
* update old references to $HOME directories to use docker volumes ([383fd41](https://github.com/spaceandtimefdn/sxt-node/commit/383fd4146dd0fa0be9dd092b602d1a49433c1fb5))
* update snapshot URL to use CDN and speed up downloads ([257304f](https://github.com/spaceandtimefdn/sxt-node/commit/257304f2391085d68bb70a7da04c46b24a632285))
* Update the contracts in the instructions to match updates ([b8015ba](https://github.com/spaceandtimefdn/sxt-node/commit/b8015baf403bf8a7ad41913cf4704d3abd5070b7))
* update wording in section 2.2 ([3ca4874](https://github.com/spaceandtimefdn/sxt-node/commit/3ca48745df44c5de2297ee62196292c4979e4111))
* update wording to mention docker volume rather than local folder ([63764b6](https://github.com/spaceandtimefdn/sxt-node/commit/63764b6fbe6f954615bf14b359928bf05b701735))
* use 3.3 instead of likely typo 2.3 ([717b4f6](https://github.com/spaceandtimefdn/sxt-node/commit/717b4f698656af8789a7524cc62ed2d02d2ced5c))
* use environment variable for validator name ([443fb49](https://github.com/spaceandtimefdn/sxt-node/commit/443fb49f1a7e2e0d93cd8cd3ca499cfb8d752967))


### Features

* add documentation for mainnet ([b3d793f](https://github.com/spaceandtimefdn/sxt-node/commit/b3d793fc6fd162389f0f1fe8aa18043debacfee7))
* add links to other sections in the doc ([a40e3ec](https://github.com/spaceandtimefdn/sxt-node/commit/a40e3ec4065dfbd3666fd9ee7492a4c3750f3b72))
* add step to copy snapshot in docker container ([84d3868](https://github.com/spaceandtimefdn/sxt-node/commit/84d3868863f207f9051afe61c195efdca16c3b60))
* add step to download validator snapshot ([480fd3d](https://github.com/spaceandtimefdn/sxt-node/commit/480fd3deef41699f104827222a3c2e85e5d67ce1))
* bump runtime version to 228 ([554f23c](https://github.com/spaceandtimefdn/sxt-node/commit/554f23c4b4ed6f3dabf2ef38963d66419e281f85))
* change default validator commission to 10% ([29819dc](https://github.com/spaceandtimefdn/sxt-node/commit/29819dc59d96bbb6325c2bb984492d898047e37f))
* Create README with testnet validator setup instructions ([7f6de3c](https://github.com/spaceandtimefdn/sxt-node/commit/7f6de3c02f35b6986c43f5272f415b4b0e1af5bc))
* Remove subkey in favor of using the sxt-node functionality ([eb35663](https://github.com/spaceandtimefdn/sxt-node/commit/eb35663f08b00bfa92108be3dfa56b8164afc71a))
* Started the FAQ section, more updates to Nominating ([25416bc](https://github.com/spaceandtimefdn/sxt-node/commit/25416bcebf82e927376eeb0b5a8bd6f2837b7c40))
* Update Code Owners ([5447594](https://github.com/spaceandtimefdn/sxt-node/commit/5447594617ae7c876d4acd3373cdbe2490ed073b))
* Update docs based on common questions and feedback ([1345df5](https://github.com/spaceandtimefdn/sxt-node/commit/1345df52773167b3d272790ab3c005e3b436d725))
* Update docs for SXT Testnet ([4001be4](https://github.com/spaceandtimefdn/sxt-node/commit/4001be4b9807b714066f92c1985dcab723aa0173))
* update hardware specs and docker image version ([8040836](https://github.com/spaceandtimefdn/sxt-node/commit/8040836a81f7b7bf5157abb4f1543a65762c974d))
* Update License ([6a9f7c8](https://github.com/spaceandtimefdn/sxt-node/commit/6a9f7c8fc72fbd6e8a8248e4e40a7ba8989b9904))



# [1.5.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.4.0...v1.5.0) (2025-05-28)


### Features

* add zkpay to system staking contracts ([4a07a0a](https://github.com/spaceandtimefdn/sxt-node/commit/4a07a0ad40c9ec7e42cc007daa5a5a6e0520fad7))
* make new namespace for zkpay ([28c77d4](https://github.com/spaceandtimefdn/sxt-node/commit/28c77d47147967b3859486479b128dd5980c126b))



# [1.4.0](https://github.com/spaceandtimefdn/sxt-node/compare/v1.3.0...v1.4.0) (2025-05-24)


### Bug Fixes

* correct build errors ([0f74bc7](https://github.com/spaceandtimefdn/sxt-node/commit/0f74bc7c50feed96c4c9c7bb700fe0315677484f))


### Features

* add reward rate calculation to canary ([5b99429](https://github.com/spaceandtimefdn/sxt-node/commit/5b99429894f9f39e7ea8deea4d7c10660c619b91))
* update chain runtime to include new rewards pallet ([bb4a46e](https://github.com/spaceandtimefdn/sxt-node/commit/bb4a46e0c0559f1a2fb1b1bebcdbc65e49ae3685))



