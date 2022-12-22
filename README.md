<div id="top"></div>
<!-- PROJECT LOGO -->
<br />
<div align="right">

  <img src="./.github/assets/logo.png#gh-light-mode-only" alt="Logo" width="200">
  <img src="./.github/assets/logo_dark.png#gh-dark-mode-only" alt="Logo" width="200">

</div>

The `topos` utility provides a unified command line interface to the [Topos](https://docs.toposware.com/general-overview) network.

[![codecov](https://codecov.io/gh/toposware/topos/branch/main/graph/badge.svg?token=FOH2B2GRL9)](https://codecov.io/gh/toposware/topos)
![example workflow](https://github.com/toposware/topos/actions/workflows/test.yml/badge.svg)
![example workflow](https://github.com/toposware/topos/actions/workflows/format.yml/badge.svg)
![example workflow](https://github.com/toposware/topos/actions/workflows/lint.yml/badge.svg)

## Building

```shell
cargo build --release
```

## Getting Started

```
# Run a local node
topos tce run
```

If you want to be part of the development, make sure to have your workflow complete.

### Testing

```
cargo test --all
```

### Formatting

```
cargo fmt --check
```

### Linting

```
cargo clippy --all
```

## Docker

The above actions can also be run in docker, using the corresponding docker `target`.

A few build arguments are required:

- GITHUB_TOKEN: PAT with `read` permission on repos
- TOOLCHAIN_VERSION: `(stable|nightly-2022-07-20|...)`

Targeted docker build commands follow the following pattern:

```
docker build . --build-arg GITHUB_TOKEN=*** --build-arg TOOLCHAIN_VERSION=[...] --target [TARGET]
```

### Build

```
docker build . ... --target build
```

### Testing

```
docker build . ... --target test
```

### Formatting

```
docker build . ... --target fmt
```

### Linting

```
docker build . ... --target lint
```

## License

This project is released under the terms of the MIT license.
