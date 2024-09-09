# Stage 1: Build environment setup with Cargo Chef for Rust dependencies
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

ARG REGTEST=false

# Install system dependencies
RUN apt-get update && apt-get -y upgrade && apt-get install -y \
    libclang-dev \
    pkg-config \
    build-essential \
    curl \
    libssl-dev \
    libffi-dev \
    libgmp-dev \


# Prepare a build plan using Cargo Chef
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Build application binaries
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Set build profile and environment variables
ARG BUILD_PROFILE=release
ENV BUILD_PROFILE=$BUILD_PROFILE

ARG RUSTFLAGS=""
ENV RUSTFLAGS="$RUSTFLAGS"

ARG FEATURES=""
ENV FEATURES=$FEATURES

# Build the dependencies
RUN cargo chef cook --profile $BUILD_PROFILE --features "$FEATURES" --recipe-path recipe.json

# Build the application binaries
COPY . .
RUN cargo build --profile $BUILD_PROFILE --features "$FEATURES" --locked -bin alpen-express-sequencer --bin alpen-express-reth

# Stage 3: Final runtime environment setup
FROM ubuntu:20.04 AS runtime
WORKDIR /app

# Environment variables for sequencer

ENV BITCOIND_HOST
ENV BITCOIND_RPC_USER
ENV BITCOIND_RPC_PASSWORD
ENV BITCOIND_RPC_PORT
ENV NETWORK
ENV KEYFILE
ENV SEQ_BTC_ADDRESS


# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    curl \
    libssl-dev \
    libffi-dev \
    && rm -rf /var/lib/apt/lists/*

RUN if [ ${NETWORK} == "regtest" ]; then \
    echo "Network is set to regtest. Installing bitcoind..."; \
    apt-get install -y bitcoind;
    ENV BITCOIND_DATADIR=/app/bitcoin
    mkdir -p /app/bitcoin
fi

# Copy the built binaries from the builder stage
COPY --from=builder /app/target/$BUILD_PROFILE/alpen-express-sequencer /usr/local/bin/alpen-express-sequencer
COPY --from=builder /app/target/$BUILD_PROFILE/alpen-express-reth /usr/local/bin/alpen-express-reth

# Expose necessary ports
# EXPOSE 30303 30303/udp 9001 8545 8546

ENTRYPOINT ["./docker-entrypoint.sh"]

