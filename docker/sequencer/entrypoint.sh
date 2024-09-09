#! /bin/bash

## Entry point for Dockerfile

# fail if env is missing
set -eu

if [[ NETWORK == "REGTEST" ]]; then
        apt-get install -y bitcoind \
        mkdir /app/bitcoind/ \
        bitcoind -regtest -txindex -printtoconsole -datadir=/app/bitcoind/ -rpcuser=$BITCOIND_RPC_USER -rpcpassword=$BITCOIND_RPC_PASSWORD -rpcport=$BITCOIN_RPC_PORT &  \
        BITCOIND_RPC_HOST=127.0.0.1 \

fi

mkdir /app/sequencer/

alpen-express-sequencer \
    --datadir /app/sequencer/ \
    --rpc-port $BITCOIND_RPC_PORT \
    --bitcoind-host $BITCOIND_HOST \
    --bitcoind-user $BITCOIND_RPC_USER \
    --bitcoind-password $BITCOIND_RPC_PASSWORD \
    --reth-authrpc reth_socket \
    --reth-jwtsecret reth_secret_path \
    --network regtest \
    --sequencer-key $SEQUENCER_KEY \
    --sequencer-bitcoin-address $SEQ_BTC_ADDRESS \
