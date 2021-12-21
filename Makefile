all: build
precommit: check test build
check: fmt-check clippy-check
fix: fmt-fix clippy-fix

build:
	RUSTFLAGS='-D warnings' cargo build

# docker image
#
# for the push to work you must be logged in
#  see https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry
#  and https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token
#
di:
	docker build --platform=linux/amd64 -t ghcr.io/toposware/tce:latest .
	docker push ghcr.io/toposware/tce:latest

test:
	RUSTFLAGS='-D warnings' cargo test --all

clippy-check:
	cargo clippy --all -- -D clippy::suspicious

clippy-fix:
	cargo clippy --fix --allow-dirty

fmt-check:
	cargo fmt --all --check

fmt-fix:
	cargo fmt --all

simu:
	cargo build --release -p topos-tce-simulation

simu-run:
	RUST_LOG=debug cargo run --release -p topos-tce-simulation

cert-spammer:
	cargo build --release -p cert-spammer

run-spam: cert-spammer
	RUST_LOG=info cargo run --release -p cert-spammer -- --target-nodes "cert-spammer/example_target_nodes.json"
