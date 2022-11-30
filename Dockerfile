ARG TOOLCHAIN_VERSION
FROM ghcr.io/toposware/rust_builder:1.65-bullseye-${TOOLCHAIN_VERSION} AS base

ARG GITHUB_TOKEN
RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"

WORKDIR /usr/src/app

FROM base AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM base AS build
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM base AS test
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --all-targets --recipe-path recipe.json
COPY . .
RUN cargo test --workspace

FROM base AS fmt
RUN rustup component add rustfmt
COPY . .
RUN cargo fmt --all -- --check

FROM base AS lint
RUN rustup component add clippy
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY . .
RUN cargo clippy --all

FROM base AS audit
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
RUN cargo install cargo-audit --locked
COPY . .
RUN cargo audit

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

COPY --from=build /usr/src/app/target/release/topos .

USER topos:topos

ENTRYPOINT ["./topos"]
