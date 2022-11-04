FROM rust:latest AS base

ARG TOOLCHAIN_VERSION
ARG GITHUB_TOKEN
ARG CODECOV_TOKEN
ARG SCCACHE_AZURE_CONNECTION_STRING
ARG SCCACHE_AZURE_BLOB_CONTAINER
ARG SCCACHE_AZURE_KEY_PREFIX

ENV CARGO_TERM_COLOR=always
ENV RUSTFLAGS=-Dwarnings
ENV RUST_BACKTRACE=1

RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"

RUN apt update && \
    apt install -y clang cmake protobuf-compiler libprotobuf-dev

RUN rustup toolchain install ${TOOLCHAIN_VERSION} && \
    rustup default ${TOOLCHAIN_VERSION} && \
    rustup target add x86_64-unknown-linux-musl && \
    cargo install sccache --locked

WORKDIR /usr/src/app

FROM base AS build
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
COPY . .
RUN cargo build --release && sccache --show-stats

FROM base AS test
COPY . .
RUN cargo test --workspace

FROM base AS fmt
RUN rustup component add rustfmt
COPY . .
RUN cargo fmt --all -- --check

FROM base AS lint
RUN rustup component add clippy
COPY . .
RUN cargo clippy --all

FROM base AS audit
RUN cargo install cargo-audit --locked
COPY . .
RUN cargo audit && sccache --show-stats

FROM debian:bullseye-slim

ENV RUST_LOG=trace
ENV TCE_PORT=9090
ENV TCE_RAM_STORAGE=true
ENV USER=topos
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /usr/src/app

COPY --from=build /usr/src/app/target/release/topos-tce-app .

USER topos:topos

CMD ./topos-tce-app
