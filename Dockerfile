ARG TOOLCHAIN_VERSION

FROM ghcr.io/toposware/rust_builder:1.65-bullseye-${TOOLCHAIN_VERSION} AS base

ARG FEATURES
ARG GITHUB_TOKEN

# Rust cache
ARG SCCACHE_S3_KEY_PREFIX=topos
ENV SCCACHE_BUCKET=cicd-devnet-1-sccache
ENV SCCACHE_REGION=us-east-1
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache

RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"

WORKDIR /usr/src/app

FROM base AS build
COPY . .
RUN --mount=type=secret,id=aws,target=/root/.aws/credentials \
  cargo build --release --no-default-features --features=${FEATURES}

FROM base AS test
RUN cargo install cargo-nextest --locked
COPY . .
# topos-sequencer integration tests require specific setup, so excluding them here. They are executed
# with sequencer_tcc_test.yml CI setup
RUN --mount=type=secret,id=aws,target=/root/.aws/credentials \
  cargo nextest run --workspace --exclude topos-sequencer-subnet-runtime-proxy --config-file tools/config/nextest.toml && cargo test --doc --workspace

FROM base AS fmt
RUN rustup component add rustfmt
COPY . .
RUN --mount=type=secret,id=aws,target=/root/.aws/credentials \
  cargo fmt --all -- --check

FROM base AS lint
RUN rustup component add clippy
COPY . .
RUN --mount=type=secret,id=aws,target=/root/.aws/credentials \
  cargo clippy --all

FROM base AS audit
RUN cargo install cargo-audit --locked
COPY . .
RUN --mount=type=secret,id=aws,target=/root/.aws/credentials \
  cargo audit

FROM debian:bullseye-slim AS topos

ENV TCE_PORT=9090
ENV USER=topos
ENV UID=10001
ENV PATH="${PATH}:/usr/src/app"

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /usr/src/app

COPY --from=build /usr/src/app/target/release/topos .
COPY tools/init.sh ./init.sh
COPY tools/liveness.sh /tmp/liveness.sh

RUN apt-get update && apt-get install -y \
    ca-certificates \
    jq \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/*

USER topos:topos

RUN mkdir /tmp/shared

ENTRYPOINT ["./init.sh"]
