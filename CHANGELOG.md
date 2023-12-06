# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2023-12-05

### Added
- Use websocket stream block listener PR#371
### Fixed
- Cleanup feature flags: Always build the full repository PR#352
### Changed
- Verify double echo messages against list of approved validator nodes PR#298
- Separate p2p and validator identity: Adding ValidatorId to double echo messages PR#319
- Cleanup CLI interface and introduce topos regtest and topos status subcommand PR#352
### Removed
