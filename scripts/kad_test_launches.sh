#! /bin/zsh

export RUST_LOG=debug

# 12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001
cargo run -- --local-key-seed 1 --tce-local-port 30001 --web-api-local-port 8011

# 12D3KooWH3uVF6wv47WnArKHk5p6cvgCJEb74UTmxztmQDc298L3 /ip4/127.0.0.1/tcp/30002
cargo run -- --local-key-seed 2 --tce-local-port 30002 --web-api-local-port 8012 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /dns4/localhost/tcp/30001"

# 12D3KooWQYhTNQdmr3ArTeUHRYzFg94BKyTkoWBDWez9kSCVe2Xo /ip4/127.0.0.1/tcp/30003
cargo run -- --local-key-seed 3 --tce-local-port 30003 --web-api-local-port 8013 --db-path "db3"  --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

# 12D3KooWLJtG8fd2hkQzTn96MrLvThmnNQjTUFZwGEsLRz5EmSzc /ip4/127.0.0.1/tcp/30004
cargo run -- --local-key-seed 4 --tce-local-port 30004 --web-api-local-port 8014 --db-path "db4" --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"

# 12D3KooWSHj3RRbBjD15g6wekV8y3mm57Pobmps2g2WJm6F67Lay /ip4/127.0.0.1/tcp/30005
cargo run -- --local-key-seed 5 --tce-local-port 30005 --web-api-local-port 8015 --boot-peers "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X /ip4/127.0.0.1/tcp/30001"
