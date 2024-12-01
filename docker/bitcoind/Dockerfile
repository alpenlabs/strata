# Use the official Ubuntu 24.04 base image
FROM ubuntu:24.04

WORKDIR /app

# Install necessary dependencies
RUN apt-get update && \
    apt-get install -y software-properties-common build-essential libtool autotools-dev automake pkg-config bsdmainutils python3 && \
    apt-get install -y libevent-dev libboost-dev libsqlite3-dev && \
    apt-get install -y libssl-dev libboost-all-dev git

# Set Git with dummy user configuration
# This configuration is needed for cherry-picking the specific commit
RUN git config --global user.email "john@example.com" && \
    git config --global user.name "John Doe"

# Clone Bitcoin repository and checkout the commit for v28.0 tag
RUN git clone https://github.com/bitcoin/bitcoin.git && \
    cd bitcoin && \
    git checkout 5de225f5c145368f70cb5f870933bcf9df6b92c8

# Apply the specific pr https://github.com/bitcoin/bitcoin/pull/27446/commits
# which includes the changes needed for configurable block time for signet
RUN cd bitcoin && \
    git fetch origin pull/27446/head && \
    git cherry-pick d8434da3c14ed6723d86ef2cd266008d366e1413

# Build Bitcoin from source
RUN cd bitcoin && \
    ./autogen.sh && \
    ./configure && \
    make && \
    make install

# Create a directory for Bitcoin data
RUN mkdir -p /root/.bitcoin

COPY entrypoint.sh entrypoint.sh
COPY bcli.sh bcli.sh

RUN chmod +x /app/entrypoint.sh && \
    chmod +x /app/bcli.sh

EXPOSE 18443 18444

# Start the bitcoind in regtest mode
ENTRYPOINT ["/app/entrypoint.sh"]
