<div id="top"></div>
<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://toposware.com/">
    <img src="https://toposware.com/logo.svg" alt="Logo" width="300" height="80">
  </a>

<h3 align="center">Transmission Control Engine</h3>

  <p align="center">
    TCE Node Implementation
  </p>
</div>


The [Transmission Control Engine](https://docs.toposware.com/learn/tce/overview) serves as the foundation for consistent cross-subnet communication, which is core to the [Topos](https://docs.toposware.com/general-overview) ecosystem.
This repository includes the core implementation of TCE client.
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

## Tools

Some tools are implemented in this repository, namely,

The [params-minimizer](./params-minimizer/) aims at figuring out the protocol parameters of the TCE. Specifically, the TCE runs on simulated environment, with various parameters in order to understand what are the optimal values.

The [cert-spammer](./cert-spammer/) aims at spamming a deployed TCE network with a constant load of dummy Certificate.

## License

This project is released under the terms of the MIT license.
