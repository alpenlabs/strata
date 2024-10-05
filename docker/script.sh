#!/bin/bash
set -e

export PATH=../target/debug/:$PATH
CLEANUP_FILE=cleaner.sh
# Base directory for data, default is 'data' if not provided
BASE_DIR=${2:-scriptData}

# clean
rm -rf $BASE_DIR
# if [ ! -z $RESTART ]; then
# fi

if [ -f $CLEANUP_FILE ]; then
    # bash $CLEANUP_FILE
    rm -rf $CLEANUP_FILE
fi

# Number of nodes to start, default is 1 if not provided
NODES=${1:-5}

export RUST_LOG=trace

mkdir -p $BASE_DIR


# Bitcoind connection info (adjust these values as needed)
BITCOIND_SOCK=${BITCOIND_SOCK:-localhost:9823}
BITCOIND_USER=${BITCOIND_USER:-alpen}
BITCOIND_PASS=${BITCOIND_PASS:-alpen}

# Sequencer RPC (adjust the port if your sequencer runs on a different port)
SEQUENCER_RPC=${SEQUENCER_RPC:-ws://localhost:12332}

# Path to JWT secret
RETH_SECRET_PATH=${RETH_SECRET_PATH:-$BASE_DIR/jwt.hex}

echo $JWTSECRET > $RETH_SECRET_PATH


# Start n full nodes and accompanying Reth nodes
for (( i=1; i<=NODES; i++ ))
do
    echo "Starting node $i..."

    RETH_DATADIR="$BASE_DIR/reth.$i"
    FULLNODE_DATADIR="$BASE_DIR/fullnode.$i"

    mkdir -p "$RETH_DATADIR"
    mkdir -p "$FULLNODE_DATADIR"

    AUTHRPC_PORT=$(( 20000 + i ))
    LISTENER_PORT=$(( 30000 + i ))
    ETHRPC_WS_PORT=$(( 40000 + i ))
    ETHRPC_HTTP_PORT=$(( 50000 + i ))
    FULLNODE_RPC_PORT=$(( 60000 + i ))

    # Start Reth node
    alpen-express-reth \
        --disable-discovery \
        --ipcdisable \
        --datadir "$RETH_DATADIR" \
        --authrpc.port "$AUTHRPC_PORT" \
        --authrpc.jwtsecret "$RETH_SECRET_PATH" \
        --port "$LISTENER_PORT" \
        --ws \
        --ws.port "$ETHRPC_WS_PORT" \
        --http \
        --http.port "$ETHRPC_HTTP_PORT" \
        --color never \
        --enable-witness-gen \
        --custom-chain "dev" \
        -vvvv \
        > "$RETH_DATADIR/service.log" &

    echo "Reth node $i started with authrpc port $AUTHRPC_PORT."
    echo kill $! >> $CLEANUP_FILE

    sleep 2

    curl -X POST \
      http://localhost:$ETHRPC_HTTP_PORT \
      -H "Content-Type: application/json" \
      -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

    # Wait for Reth node to start
    sleep 5

    # Reth authrpc socket
    RETH_AUTHRPC="localhost:$AUTHRPC_PORT"


    # Start full node
    alpen-express-sequencer \
        --datadir "$FULLNODE_DATADIR" \
        --rpc-host "localhost" \
        --rpc-port "$FULLNODE_RPC_PORT" \
        --bitcoind-host "$BITCOIND_SOCK" \
        --bitcoind-user "$BITCOIND_USER" \
        --bitcoind-password "$BITCOIND_PASS" \
        --reth-authrpc "$RETH_AUTHRPC" \
        --reth-jwtsecret "$RETH_SECRET_PATH" \
        --network "regtest" \
        --sequencer-rpc "$SEQUENCER_RPC" \
        > "$FULLNODE_DATADIR/service.log" &
    echo "Full node $i started with RPC port $FULLNODE_RPC_PORT."
    echo kill $! >> $CLEANUP_FILE
done

echo "Started $NODES full nodes and their accompanying Reth nodes."

sleep 1000
