# [0.15.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.14.0...v0.15.0) (2024-10-10)


### Features

* upgrade proof-of-sql to 0.28.10 ([68335d0](https://github.com/spaceandtimelabs/sxt-node/commit/68335d0980c2c85ceb2cb044ac2826a76765d8a1))



# [0.14.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.13.0...v0.14.0) (2024-10-10)


### Features

* add TableCommitmentBytesPerCommitmentScheme type alias ([0727aa5](https://github.com/spaceandtimelabs/sxt-node/commit/0727aa5e6487cb8f5ef450fb37471556ae0465aa))
* derive codec traits for generic over commitment types ([24f8055](https://github.com/spaceandtimelabs/sxt-node/commit/24f805503ed35e08fa99a60b5f19f63a5a2f4b07))



# [0.13.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.12.0...v0.13.0) (2024-10-10)


### Features

* derive Eq for commitment-map KeyExistsError ([a62a19a](https://github.com/spaceandtimelabs/sxt-node/commit/a62a19a320315ef6d2793629b55458c0ec98cbed))
* implement pallet-commitments storage and limited API ([c345e6f](https://github.com/spaceandtimelabs/sxt-node/commit/c345e6f48e84f89f64a4bbf77a72f7513b36c86b))
* integrate commitments pallet into runtime ([3e1f9de](https://github.com/spaceandtimelabs/sxt-node/commit/3e1f9dee71ea174d5d1e377c3027db70b996687c))



# [0.12.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.11.0...v0.12.0) (2024-10-09)


### Features

* store unhashed table commitment bytes ([2925419](https://github.com/spaceandtimelabs/sxt-node/commit/2925419b6ee27d2402f81ac9d3c9c0eb3b94a16f))



# [0.11.0](https://github.com/spaceandtimelabs/sxt-node/compare/v0.10.0...v0.11.0) (2024-10-08)


### Features

* add commitment scheme mapping API ([ef4a2eb](https://github.com/spaceandtimelabs/sxt-node/commit/ef4a2eb08009580051cb84681459e0fb0c312ba0))
* add commitment scheme zip and unzip APIs ([5936733](https://github.com/spaceandtimelabs/sxt-node/commit/5936733f2fcf00b9fb66a69868cb6672a0d19c9d))
* add PerCommitmentScheme::select method ([161d788](https://github.com/spaceandtimelabs/sxt-node/commit/161d78821b48f51c44a6d0aa807e5988847fdfba))
* allow non-generics in commitment scheme types ([8b4bb27](https://github.com/spaceandtimelabs/sxt-node/commit/8b4bb27d6579386ecddf81a52683a0eb40a53723))
* allow Results in commitment scheme types ([62cf223](https://github.com/spaceandtimelabs/sxt-node/commit/62cf2236c74a5c7f8ed2dbd330786888d7db573f))
* implement Default for PerCommitmentScheme<OptionType<T>> ([c70df2b](https://github.com/spaceandtimelabs/sxt-node/commit/c70df2beae2e4181398f5da2b3ff1392dbfb7d00))
* implement FromIterator for PerCommitmentScheme<OptionType<T>> ([a669758](https://github.com/spaceandtimelabs/sxt-node/commit/a669758c6f8a5256ce7639b09421c6329a34b063))
* promote optional PerCommitmentScheme fields to higher kinded types ([9fc34d9](https://github.com/spaceandtimelabs/sxt-node/commit/9fc34d9d249a344edadc94d695c00d5f5784e21d))



