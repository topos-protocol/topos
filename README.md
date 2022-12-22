<div id="top"></div>
<!-- PROJECT LOGO -->
<br />
<div align="right">

  <img src="./.github/assets/logo.png#gh-light-mode-only" alt="Logo" width="250">
  <img src="./.github/assets/logo_dark.png#gh-dark-mode-only" alt="Logo" width="250">

</div>

<br />

**`topos` is the unified command line interface to the [Topos](https://docs.toposware.com/general-overview) network.**

[![codecov](https://codecov.io/gh/toposware/topos/branch/main/graph/badge.svg?token=FOH2B2GRL9&style=flat)](https://codecov.io/gh/toposware/topos)
![example workflow](https://github.com/toposware/topos/actions/workflows/test.yml/badge.svg)
![example workflow](https://github.com/toposware/topos/actions/workflows/format.yml/badge.svg)
![example workflow](https://github.com/toposware/topos/actions/workflows/lint.yml/badge.svg)
<!-- [![](https://dcbadge.vercel.app/api/server/INVITEID)](https://discord.gg/INVITEID?style=flat) -->


## Getting Started

**Install Rust**

The first step is to install Rust along with `cargo` by following [those instructions](https://doc.rust-lang.org/book/ch01-01-installation.html#installing-rustup-on-linux-or-macos).

**Install `topos`**

```
cargo install topos --git https://github.com/toposware/topos
```

**Try out `topos`!**
```
topos --help
```

## Development

If you want to be part of the development, make sure to have your workflow complete.

```
# Building
cargo build

# Testing
cargo test --workspace

# Formatting
cargo fmt --check

# Linting
cargo clippy --all
```

The workflow with docker and docker compose are described on `./tools/README.md`

## Contributions and support

Contributions are pretty welcomed, you can reach the contributing guidelines in [`CONTRIBUTING.md`](./CONTRIBUTING.md).<br />
Feel free to [open an issue](https://github.com/toposware/topos/issues/new) if you have any feature request or bug report.<br />
If you have any questions, do not hesitate to reach us on [Discord](https.//discord.com/)!

## License

This project is released under the terms of the MIT license.
