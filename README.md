<div id="top"></div>
<!-- PROJECT LOGO -->
<br />
<div align="center">

  <img src="./.github/assets/logo.png#gh-light-mode-only" alt="Logo" width="300">
  <img src="./.github/assets/logo_dark.png#gh-dark-mode-only" alt="Logo" width="300">

  <h1>Transmission Control Engine</h1>
  
  <p>
    The <a href="https://docs.toposware.com/learn/tce/overview" target="_blank">Transmission Control Engine</a> serves as the foundation for consistent cross-subnet communication, <br/> which is core to the <a href="https://docs.toposware.com/general-overview" target="_blank">Topos</a> ecosystem.
  </p>
  
  [![codecov](https://codecov.io/gh/toposware/tce/branch/main/graph/badge.svg?token=FOH2B2GRL9)](https://codecov.io/gh/toposware/tce)
  ![example workflow](https://github.com/toposware/tce/actions/workflows/test.yml/badge.svg)
  ![example workflow](https://github.com/toposware/tce/actions/workflows/format.yml/badge.svg)
  ![example workflow](https://github.com/toposware/tce/actions/workflows/lint.yml/badge.svg)
</div>

## Build

```shell
cargo build --release
```

## Development

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

## Tools

Some tools are implemented in this repository, namely,

The [params-minimizer](./params-minimizer/) aims at figuring out the protocol parameters of the TCE. Specifically, the TCE runs on simulated environment, with various parameters in order to understand what are the optimal values.

## License

This project is released under the terms specified in the [LICENSE](LICENSE) file.
