![topos](./.github/assets/topos_logo_dark.png)
## [0.0.11](https://github.com/topos-protocol/topos/compare/v0.0.10..v0.0.11) - 2024-02-08

### ⛰️  Features

- Introduce topos-config crate ([#443](https://github.com/topos-protocol/topos/issues/443)) - ([4ff2a23](https://github.com/topos-protocol/topos/commit/4ff2a23e3a05ea3e950763bd4bde3d3ef6ef891b))
- Adding positions to certificate ([#440](https://github.com/topos-protocol/topos/issues/440)) - ([5315710](https://github.com/topos-protocol/topos/commit/531571025a4d81f9d9aa713ca12594756ca56a7e))
- Improve sequencer error handling and shutdown/restart sequence ([#428](https://github.com/topos-protocol/topos/issues/428)) - ([ab8bb9e](https://github.com/topos-protocol/topos/commit/ab8bb9e83afee545c3730f974ae8591c7fc70f3d))
- Update double echo to use pending CF ([#418](https://github.com/topos-protocol/topos/issues/418)) - ([8fb4003](https://github.com/topos-protocol/topos/commit/8fb4003d5579a8fee6d81c463707131959f076c3))
- Use anvil for sequencer tests ([#427](https://github.com/topos-protocol/topos/issues/427)) - ([5b0257b](https://github.com/topos-protocol/topos/commit/5b0257bed685c064c3eafbea2b5c77125e6c9041))
- Update tce config addresses ([#415](https://github.com/topos-protocol/topos/issues/415)) - ([476948f](https://github.com/topos-protocol/topos/commit/476948fa671b431bfa797aabf6b96949ac734db6))
- Remove the register commands macro ([#426](https://github.com/topos-protocol/topos/issues/426)) - ([985d0be](https://github.com/topos-protocol/topos/commit/985d0be0c75ddd1d41e94a172824b706ca0f9c5f))
- Run e2e topos integration workflow ([#408](https://github.com/topos-protocol/topos/issues/408)) - ([f0b7637](https://github.com/topos-protocol/topos/commit/f0b763786aa869454c9e30076ed08d0a456ed319))
- Adding filter on message when non validator ([#405](https://github.com/topos-protocol/topos/issues/405)) - ([b096482](https://github.com/topos-protocol/topos/commit/b0964825a5f386d75507482ee9068e26c9d74fe0))
- Add no-edge-process flag to node init ([#401](https://github.com/topos-protocol/topos/issues/401)) - ([28a553b](https://github.com/topos-protocol/topos/commit/28a553b6d17933bfbcca835640cc0c165bcd0124))
- Refactor peer selection for synchronization ([#382](https://github.com/topos-protocol/topos/issues/382)) - ([6982d33](https://github.com/topos-protocol/topos/commit/6982d336296a9b9ec5eacb025d938b6cb47b6e0a))
- Remove task manager channels ([#391](https://github.com/topos-protocol/topos/issues/391)) - ([f5fa427](https://github.com/topos-protocol/topos/commit/f5fa4276d8a524fd04bbf2a0d1d036d7f5af34bb))
- Add batch message and update double echo ([#383](https://github.com/topos-protocol/topos/issues/383)) - ([f0bc90c](https://github.com/topos-protocol/topos/commit/f0bc90c7480a84c0c12016e748f2a002477f4417))

### 🐛 Bug Fixes

- Fixing wrong use of IntCounterVec ([#442](https://github.com/topos-protocol/topos/issues/442)) - ([fe062a5](https://github.com/topos-protocol/topos/commit/fe062a5bdfa9ade2b94de88cbd6b3946b72a94c3))
- Clippy unused ([#434](https://github.com/topos-protocol/topos/issues/434)) - ([4aa6a9e](https://github.com/topos-protocol/topos/commit/4aa6a9e4723b8aaa2c2da0fdf32d8c8c926f7764))
- Remove an unused channel that was locking the broadcast ([#433](https://github.com/topos-protocol/topos/issues/433)) - ([43c6fe5](https://github.com/topos-protocol/topos/commit/43c6fe5caffd35103b20ec11d7ae2f35b1af98b9))
- RUSTSEC-2024-0003 ([#431](https://github.com/topos-protocol/topos/issues/431)) - ([cad4d76](https://github.com/topos-protocol/topos/commit/cad4d76ccf88be3f1399d371cf9dbdd7211343bc))
- RUSTSEC-2023-0078 ([#429](https://github.com/topos-protocol/topos/issues/429)) - ([35f8930](https://github.com/topos-protocol/topos/commit/35f8930056f641b33c56b4799b8c0cbe6f0a5eda))
- Fixing release notification ([#423](https://github.com/topos-protocol/topos/issues/423)) - ([7503fa7](https://github.com/topos-protocol/topos/commit/7503fa7f2385294f2a12a86eaa521af61fb9bc95))
- Move test abi generation to separate module ([#424](https://github.com/topos-protocol/topos/issues/424)) - ([d4ff358](https://github.com/topos-protocol/topos/commit/d4ff3581d43eabb14c3d977f34b69aee396e98d4))
- Return error from subprocesses ([#422](https://github.com/topos-protocol/topos/issues/422)) - ([53b3229](https://github.com/topos-protocol/topos/commit/53b3229b7e26b7bb9b3a8d97725e7cc86174df9b))

### 🚜 Refactor

- Graphql types to differentiate inputs ([#435](https://github.com/topos-protocol/topos/issues/435)) - ([4b0ec9b](https://github.com/topos-protocol/topos/commit/4b0ec9b2b3b6ab075d1b9bfde54dc8bb179bbde8))

### ⚙️ Miscellaneous Tasks

- Debug 0.0.11 synchronization ([#447](https://github.com/topos-protocol/topos/issues/447)) - ([edf86ee](https://github.com/topos-protocol/topos/commit/edf86ee32f8b34c5b11eecd5b0ed6fe5bdd0191a))
- Update dependencies ([#450](https://github.com/topos-protocol/topos/issues/450)) - ([62126e0](https://github.com/topos-protocol/topos/commit/62126e0417d8d4225eb8bb6eebb7fc0a0f526cc6))
- Remove mention of topos-api from coverage YAML ([#438](https://github.com/topos-protocol/topos/issues/438)) - ([6c7e342](https://github.com/topos-protocol/topos/commit/6c7e342715d86bcbd75e1bcd4054eb4cbeddf5cf))
- Bump version for topos to 0.0.11 ([#439](https://github.com/topos-protocol/topos/issues/439)) - ([917eaf9](https://github.com/topos-protocol/topos/commit/917eaf993336780a373dbcdfea4dd0d8b3e81f50))
- Refactor struct in topos-core ([#437](https://github.com/topos-protocol/topos/issues/437)) - ([acedac7](https://github.com/topos-protocol/topos/commit/acedac7e09094364b5406ee72f9a65241784c478))
- Update crates structure for api/uci and core ([#436](https://github.com/topos-protocol/topos/issues/436)) - ([355b08a](https://github.com/topos-protocol/topos/commit/355b08acf91052564dae65872904950d20b72ebd))
- Fix logs for pending ([#432](https://github.com/topos-protocol/topos/issues/432)) - ([342b2c7](https://github.com/topos-protocol/topos/commit/342b2c71ef0621a93b5f4460abd313f1c8b4c62b))
- Updating double echo for devnet ([#416](https://github.com/topos-protocol/topos/issues/416)) - ([1e91086](https://github.com/topos-protocol/topos/commit/1e91086a68ec01d304c5d8867e8fbcd671798599))
- Fixing clippy warning for 1.75.0 ([#425](https://github.com/topos-protocol/topos/issues/425)) - ([22a3745](https://github.com/topos-protocol/topos/commit/22a374506e087ab9cba68f9e0ed00682df6be6df))
- Refactor push certificate tests and cleanup regtest ([#399](https://github.com/topos-protocol/topos/issues/399)) - ([c60170d](https://github.com/topos-protocol/topos/commit/c60170dc19a9add84648afaf7962252ec77c160f))
- Update signal handle by tce ([#417](https://github.com/topos-protocol/topos/issues/417)) - ([beca28b](https://github.com/topos-protocol/topos/commit/beca28ba224b4a217a147f88142692acd1613a32))
- Cleanup topos tools ([#397](https://github.com/topos-protocol/topos/issues/397)) - ([0820306](https://github.com/topos-protocol/topos/commit/08203062a1cada7470d8b8207539d7a660ece466))
- Adding context on connection to self ([#413](https://github.com/topos-protocol/topos/issues/413)) - ([6e72999](https://github.com/topos-protocol/topos/commit/6e729992202ed4c56a1564cc39755ff1e0766ff8))
- Adding context on connection to self ([#411](https://github.com/topos-protocol/topos/issues/411)) - ([3a799ac](https://github.com/topos-protocol/topos/commit/3a799ac41542bd216a0512d9cf3a634a6ed2af07))
- Adding context to p2p msg received ([#410](https://github.com/topos-protocol/topos/issues/410)) - ([e1b2ccf](https://github.com/topos-protocol/topos/commit/e1b2ccf99a114a60e04c8ed187a8c9bd2cf066e4))
- Adding context to certificate broadcast ([#409](https://github.com/topos-protocol/topos/issues/409)) - ([d170b6b](https://github.com/topos-protocol/topos/commit/d170b6b386005d44457979bcd0a0f2435153f3f2))
- Adding storage context on startup ([#404](https://github.com/topos-protocol/topos/issues/404)) - ([ffae4c6](https://github.com/topos-protocol/topos/commit/ffae4c63d00bb099a49216c2af560f6b59601133))
- Remove audit ignore report ([#407](https://github.com/topos-protocol/topos/issues/407)) - ([70bce47](https://github.com/topos-protocol/topos/commit/70bce479d16c750ff1a322db1e88bfae08303855))
- Update SynchronizerService to remove unwrap ([#403](https://github.com/topos-protocol/topos/issues/403)) - ([b424aa9](https://github.com/topos-protocol/topos/commit/b424aa91f68197134faec742a23eec513cde837f))
- Adding no-color option to CLI ([#402](https://github.com/topos-protocol/topos/issues/402)) - ([4989936](https://github.com/topos-protocol/topos/commit/4989936ae000a85897d805453c91a53f4edd6580))
- Adding cross compilation ([#400](https://github.com/topos-protocol/topos/issues/400)) - ([887762f](https://github.com/topos-protocol/topos/commit/887762f740241d49d2886cd55d6a9da1d3d83e7e))
- Adding openssl as dependency ([#396](https://github.com/topos-protocol/topos/issues/396)) - ([60f873c](https://github.com/topos-protocol/topos/commit/60f873c619b8132a2276634205e3c39561eb3faf))
- Update release action target ([#395](https://github.com/topos-protocol/topos/issues/395)) - ([54db400](https://github.com/topos-protocol/topos/commit/54db40020e8e22d7b100b0b6944a45485e2939a4))
- Update release action target ([#394](https://github.com/topos-protocol/topos/issues/394)) - ([f0f28c3](https://github.com/topos-protocol/topos/commit/f0f28c33a5332232b2921b13051ae5de714cf58b))
- Adding aarch64 image ([#393](https://github.com/topos-protocol/topos/issues/393)) - ([9f48dc8](https://github.com/topos-protocol/topos/commit/9f48dc88582bfbc71cabb2c6bbc27297cc9b87ee))
- Cleanup topos test network setup ([#390](https://github.com/topos-protocol/topos/issues/390)) - ([2820664](https://github.com/topos-protocol/topos/commit/2820664a66bdfe039f1aaa80d79f4ff339c8a65c))


