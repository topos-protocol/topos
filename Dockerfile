ARG TOOLCHAIN_VERSION
FROM --platform=${BUILDPLATFORM:-linux/amd64} ghcr.io/topos-protocol/rust_builder:bullseye-${TOOLCHAIN_VERSION} AS base

ARG FEATURES
# Rust cache
ARG SCCACHE_S3_KEY_PREFIX
ARG SCCACHE_BUCKET
ARG SCCACHE_REGION
ARG RUSTC_WRAPPER
ARG PROTOC_VERSION=22.2

WORKDIR /usr/src/app

FROM --platform=${BUILDPLATFORM:-linux/amd64} base AS build
COPY . .
RUN --mount=type=secret,id=aws,target=/root/.aws/credentials \
    --mount=type=cache,id=sccache,target=/root/.cache/sccache \
  cargo build --release --no-default-features --features=${FEATURES} \
  && sccache --show-stats

FROM --platform=${BUILDPLATFORM:-linux/amd64} debian:bullseye-slim AS topos

ENV TCE_PORT=9090
ENV USER=topos
ENV UID=10001
ENV PATH="${PATH}:/usr/src/app"

WORKDIR /usr/src/app

COPY --from=build /usr/src/app/target/release/topos .

RUN apt-get update && apt-get install -y \
    ca-certificates \
    jq \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/*

RUN mkdir /tmp/node_config
RUN mkdir /tmp/shared

ENTRYPOINT ["./topos"]
