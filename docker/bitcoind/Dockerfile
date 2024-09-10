# Use the official Ubuntu 22.04 base image
FROM ubuntu:24.04

WORKDIR /app

# Install necessary dependencies and add PPA
RUN apt-get update && \
    apt-get install -y software-properties-common && \
    add-apt-repository ppa:luke-jr/bitcoincore && \
    apt-get update && \
    apt-get install -y \
    bitcoind \
    curl && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*


# Create a directory for Bitcoin data
RUN mkdir -p /root/.bitcoin

COPY ./docker/bitcoind/entrypoint.sh entrypoint.sh

RUN chmod +x /app/entrypoint.sh

EXPOSE $BITCOIND_RPC_PORT

# Start the bitcoind in regtest mode
CMD ["/app/entrypoint.sh"]
