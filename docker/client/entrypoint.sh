#! /bin/bash

## Entry point for Dockerfile

# Fail if any required environment variable is missing or any command fails
set -eu

wait_and_set_btc_addr() {
    for t in $(seq 1 10);
    do
        if [[ -f /root/.bitcoin/bitcoin-address ]];
        then
            BITCOIN_ADDR=$(cat /root/.bitcoin/bitcoin-address)
            return 0
        fi
        sleep 1
    done
    return 1
}

# Create necessary directories
mkdir -p /app/sequencer/

echo "$SECRETKEY" > seq_key.bin
echo "$JWTSECRET" > jwt.hex

if [ $NETWORK == "regtest" ]; then
    echo "Network is set to regtest"
    wait_and_set_btc_addr
    if [ $? -eq 1 ]; then
        echo "cannot set wallet address. Exiting"
        exit
    fi
else
    BITCOIN_ADDR=$BTC_ADDRESS
fi

RETH_URL="$RETH_HOST:$RETH_PORT"
BITCOIND_URL="$BITCOIND_HOST:$BITCOIND_RPC_PORT/wallet/$BITCOIND_WALLET"
# Configuration based on different modes
cat <<EOF > config.toml
[bitcoind_rpc]
rpc_url = "$BITCOIND_URL"
rpc_user = "$BITCOIND_RPC_USER"
rpc_password = "$BITCOIND_RPC_PASSWORD"
network = "$NETWORK"

[sync]
l1_follow_distance = $L1_FOLLOW_DISTANCE
max_reorg_depth = $MAX_REORG_DEPTH
client_poll_dur_ms = $CLIENT_POLL_DUR_MS
client_checkpoint_interval = $CHECKPOINT_INTERVAL

[exec.reth]
rpc_url = "$RETH_URL"
secret = "jwt.hex"

[client]
rpc_host = $RPC_HOST
rpc_port = $RPC_PORT
l2_blocks_fetch_limit = $L2_BLOCKS_FETCH_LIMIT
sequencer_bitcoin_address = "$BITCOIN_ADDR"
datadir = "/app/sequencer"
db_retry_count = $DB_RETRY_COUNT
EOF

if [[ $CLIENT_MODE == "sequencer" ]];then
    echo 'sequencer_key = "seq_key.bin"' >> config.toml
else
    echo "sequencer_rpc = $SEQUENCER_RPC" >> config.toml
fi

if [ ! -f /app/config.toml ]; then
    echo "no config toml found"
fi

cat config.toml

echo $RETH_URL
echo $BITCOIND_URL
export RUST_LOG=info

# Start the Alpen Express Sequencer
strata-sequencer \
    --config config.toml $@
