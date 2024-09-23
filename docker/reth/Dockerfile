FROM rust:latest AS builder

WORKDIR /app

# Set environment variables for optimized release builds
ENV CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=always \
    TARGET=x86_64-unknown-linux-musl

# Install system dependencies
RUN apt-get update && apt-get -y upgrade && apt-get install -y \
    pkg-config \
    build-essential \
    libclang-dev

COPY . .

# Build dependencies in release mode
RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=private,target=/app/target \
    cargo fetch

RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=private,target=/app/target \
    cargo build --release --bin alpen-express-reth


RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=private,target=/app/target \
    cp /app/target/release/alpen-express-reth /app/strata-reth

FROM ubuntu:24.04 AS runtime
WORKDIR /app

RUN apt-get update && \
    apt-get install -y \
    curl && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Copy the built binaries from the builder stage
COPY --from=builder /app/strata-reth /usr/local/bin/strata-reth

COPY ./docker/reth/entrypoint.sh entrypoint.sh

RUN chmod +x /app/entrypoint.sh

EXPOSE $RETH_AUTH_RPC_PORT $RETH_PORT

ENTRYPOINT ["/app/entrypoint.sh"]
