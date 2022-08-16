FROM rust:latest AS base

ARG TOOLCHAIN_VERSION
ARG GITHUB_TOKEN

ENV CARGO_TERM_COLOR=always
ENV RUSTFLAGS=-Dwarnings
ENV RUST_BACKTRACE=1

RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"

RUN apt update && \
    apt install -y clang

RUN rustup toolchain install ${TOOLCHAIN_VERSION} && \
    rustup default ${TOOLCHAIN_VERSION} && \
    rustup target add x86_64-unknown-linux-musl

WORKDIR /usr/src/app

COPY ./Cargo.toml ./Cargo.toml
COPY ./node/api/Cargo.toml ./node/api/Cargo.toml
COPY ./node/net/Cargo.toml ./node/net/Cargo.toml
COPY ./node/store/Cargo.toml ./node/store/Cargo.toml
COPY ./node/telemetry/Cargo.toml ./node/telemetry/Cargo.toml
COPY ./protocols/reliable_broadcast/Cargo.toml ./protocols/reliable_broadcast/Cargo.toml
COPY ./protocols/transport/Cargo.toml ./protocols/transport/Cargo.toml
COPY ./params-minimizer/Cargo.toml ./params-minimizer/Cargo.toml
COPY ./tests ./tests


FROM base as build
RUN cargo fetch
COPY ./ .
RUN cargo build --release

FROM base as test
RUN cargo fetch
COPY ./ .
RUN cargo test --workspace

FROM base as fmt
RUN rustup component add rustfmt
COPY ./ .
RUN cargo fmt --all -- --check

FROM base as lint
RUN rustup component add clippy
RUN cargo fetch
COPY ./ .
RUN cargo clippy --all

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

COPY --from=build /usr/src/app/target/release/topos-tce-node-app .

USER topos:topos

CMD topos-tce-node-app
