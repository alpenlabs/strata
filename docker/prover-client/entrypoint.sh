#!/bin/bash

# Exit on error
set -e

echo "starting Prover client"

strata-prover-client \
    --rpc-port 9851 \
    --enable-dev-rpcs true \
    --enable-checkpoint-runner false \
    --sequencer-rpc http://sequencer:8432 \
    --bitcoind-url http://bitcoind:8332 \
    --bitcoind-user $BITCOIND_USER \
    --bitcoind-password", $BITCOIND_PASSWORD \
    --reth-rpc http://reth:8545 $@
