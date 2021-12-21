# Docker Compose for TCE

## DNS in LibP2P

LibP2P Traditionally has support for DNS based resolution for node seeding and includes MultiAddr protocol blocks for DNS4 and similar to enable such.

The Rust implementation of LibP2P has a pluggable model for transport definitions so enabling one or more of the DNS supports is required to be able to parse MultiAddrs that want to specify DNS, for example:

```txt
/dns4/mycustomname/tcp/30001
```

The feature(s) needed can be found by inspecting the LibP2P source and root [Cargo.toml](https://github.com/libp2p/rust-libp2p/blob/master/Cargo.toml#L108) for hints but documentation is limited:

```toml
features = [
    "dns-async-std",
    "dns-tokio",
    "mdns",
]
```

## DNS Naming vs Static IPs

Docker compose services typically enable a pseudo dns like name resolution that enables services to reference one another but we were unable to acheive peer resolution using this mechanism even after (assumedly) configuring DNS support for LibP2P which led to this long standing issue in Docker Compose:

[DNS Resolution Fails when Container Started Via Docker Compose](https://devops.stackexchange.com/questions/14881/dns-resolution-fails-when-container-is-started-via-docker-compose)

During diagnostics to determine root cause and rule out the network we were able to achieve peering via a Static IP for each container inside a custom Virtual Subnet / CIDR block:

[Setting up static networking in Docker Compose](https://stackoverflow.com/questions/39493490/provide-static-ip-to-docker-containers-via-docker-compose)

## DNS Support for Native Docker

One could derive a script leveraging DNS outside of Docker Compose using the "--dns" flag if that was preferrable, though there are pros and cons to each:

```sh
docker run --rm -it --dns -p 1000:9090 -p 8080:8080 temp
```

Such a script could leverage a similar setup to below:

```sh
RUST_LOG=trace cargo run -- --local-key-seed 1 --tce-local-port 30001

RUST_LOG=trace cargo run -- --local-key-seed 2 --tce-local-port 30002 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

RUST_LOG=trace cargo run -- --local-key-seed 3 --tce-local-port 30003 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

RUST_LOG=trace cargo run -- --local-key-seed 4 --tce-local-port 30004 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

RUST_LOG=trace cargo run -- --local-key-seed 5 --tce-local-port 30005 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

RUST_LOG=trace cargo run -- --local-key-seed 6 --tce-local-port 30006 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"
```