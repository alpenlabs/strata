# Stage 1: Build Bitcoin from source
FROM ubuntu:24.04 AS build

WORKDIR /build

# Install build dependencies
RUN apt-get update && \
    apt-get install -y software-properties-common build-essential libtool autotools-dev automake pkg-config bsdmainutils python3 \
    libevent-dev libboost-dev libsqlite3-dev libssl-dev libboost-all-dev git

# Clone Bitcoin repository and checkout the commit for v28.0 tag
RUN git clone -b 'v28.0' --single-branch --depth 1 https://github.com/bitcoin/bitcoin.git

# Apply a diff file to the Bitcoin source code to enable configurable block
# time for signet. The diff file is generated from the following PR:
# https://github.com/bitcoin/bitcoin/pull/27446/commits
COPY configurable_blocktime_patch.diff configurable_blocktime_patch.diff

# Apply the patch to Bitcoin source code
RUN cd bitcoin && git apply /build/configurable_blocktime_patch.diff

# Build Bitcoin from source
# Run the build in parallel using one less than the number of available CPUs
RUN cd bitcoin && \
    ./autogen.sh && \
    ./configure && \
    make -j$(($(nproc) - 1)) && \
    make install

# Stage 2: Create a minimal runtime image
FROM ubuntu:24.04 AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y libevent-dev libboost-dev libsqlite3-dev libssl-dev && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# Copy the Bitcoin binaries from the build stage
COPY --from=build /usr/local/bin/bitcoind /usr/local/bin/bitcoind
COPY --from=build /usr/local/bin/bitcoin-cli /usr/local/bin/bitcoin-cli

# Create a directory for Bitcoin data
RUN mkdir -p /root/.bitcoin

# Copy entrypoint and helper scripts
COPY entrypoint.sh entrypoint.sh
COPY bcli.sh bcli.sh

RUN chmod +x /app/entrypoint.sh && \
    chmod +x /app/bcli.sh

# Expose necessary ports
EXPOSE 18443 18444

# Set the entrypoint script
ENTRYPOINT ["/app/entrypoint.sh"]
