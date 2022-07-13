#
# Builder
#
FROM --platform=linux/amd64 rust:slim-bullseye AS builder

RUN rustup toolchain install nightly && \
    rustup default nightly && \
    rustup target add x86_64-unknown-linux-musl && \
    rustup component add rustfmt
RUN apt update && \
    apt install -y clang musl-tools musl-dev sudo
RUN sudo ln -s /usr/bin/g++ /usr/bin/musl-g++
RUN update-ca-certificates

# Create appuser
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

WORKDIR /src/github.com/toposware/tce


# fetch and save dependencies as a layer
#
COPY ./Cargo.toml ./Cargo.toml
COPY ./uci/Cargo.toml ./uci/Cargo.toml
COPY ./node/api/Cargo.toml ./node/api/Cargo.toml
COPY ./node/net/Cargo.toml ./node/net/Cargo.toml
COPY ./node/store/Cargo.toml ./node/store/Cargo.toml
COPY ./node/telemetry/Cargo.toml ./node/telemetry/Cargo.toml
COPY ./protocols/reliable_broadcast/Cargo.toml ./protocols/reliable_broadcast/Cargo.toml
COPY ./protocols/transport/Cargo.toml ./protocols/transport/Cargo.toml
COPY ./params-minimizer/Cargo.toml ./params-minimizer/Cargo.toml
COPY ./cert-spammer/Cargo.toml ./cert-spammer/Cargo.toml

RUN cargo fetch

# sources and build
#
COPY ./ .
RUN cargo build --release

#
# Final image
#
FROM --platform=linux/amd64 debian:bullseye-slim

ENV RUST_LOG=trace
ENV TCE_PORT=9090
ENV TCE_RAM_STORAGE=true

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /src/github.com/toposware/tce

COPY --from=builder /src/github.com/toposware/tce/target/release/topos-tce-node-app ./topos-tce-node-app

# Use an unprivileged user.
USER topos:topos

CMD ["./topos-tce-node-app"]
