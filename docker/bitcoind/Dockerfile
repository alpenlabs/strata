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

COPY entrypoint.sh entrypoint.sh
COPY bcli.sh bcli.sh

RUN chmod +x /app/entrypoint.sh && \
    chmod +x /app/bcli.sh

EXPOSE 18443 18444

# Start the bitcoind in regtest mode
ENTRYPOINT ["/app/entrypoint.sh"]
