#! /bin/bash

## Entry point for Dockerfile

# fail if env is missing
set -eu

# ENV BITCOIND_HOST
# ENV BITCOIND_RPC_USER
# ENV BITCOIND_RPC_PASSWORD
# ENV BITCOIND_RPC_PORT
# ENV NETWORK
# ENV KEYFILE
# ENV SEQ_BTC_ADDRESS


alpen-express-sequencer \
    --datadir $BITCOIND_DATADIR \
    --rpc-port $BITCOIND_RPC_PORT \
    --bitcoind-host $BITCOIND_HOST \
    --bitcoind-user $BITCOIND_RPC_USER \
    --bitcoind-password $BITCOIND_RPC_PASSWORD \
    --reth-authrpc reth_socket \
    --reth-jwtsecret reth_secret_path \
    --network regtest \
    --sequencer-key $SEQUENCER_KEY \
    --sequencer-bitcoin-address $SEQ_BTC_ADDRESS \

if [[ NETWORK == "REGTEST" ]]; then
        bitcoind -regtest -txindex -printtoconsole -datadir=$BITCOIND_DATADIR -rpcuser=$BITCOIND_RPC_USER -rpcpassword=$BITCOIND_RPC_PASSWORD -rpcport=$BITCOIN_RPC_PORT &
fi



# alpen-express-reth
# --disable-discovery \
# --datadir $RETH_DATADIR \
# --authrpc.port $RETH_PORT \
# --authrpc.jwtsecret $RETH_SECRET_PATH \
# --port $LISTENER_PORT \
# --ws
# --ws.port str(ethrpc_ws_port) \
# --http
# --http.port str(ethrpc_http_port) \
# --color never \
# --enable-witness-gen
# -vvvv
