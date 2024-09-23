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

# Create a new empty shell project to cache dependencies

COPY . .
# Build dependencies in release mode

RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=private,target=/app/target \
    cargo build --release --bin alpen-express-sequencer

RUN --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,sharing=private,target=/app/target \
    cp /app/target/release/alpen-express-sequencer /app/strata-sequencer

FROM ubuntu:24.04 AS runtime
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    curl \
    iproute2 \
    net-tools \
    libssl-dev \
    libffi-dev \
    software-properties-common && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Copy the built binaries and the entrypoint script from the builder stage
COPY --from=builder /app/strata-sequencer /usr/local/bin/strata-sequencer
COPY ./docker/client/entrypoint.sh entrypoint.sh

EXPOSE $RPC_PORT

# Make the entrypoint script executable
RUN chmod +x entrypoint.sh

# Set the entrypoint
ENTRYPOINT ["/app/entrypoint.sh"]
