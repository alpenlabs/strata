#!/bin/bash

# Exit on error
set -e

echo "starting Prover client"

strata-prover-client \
    --rpc-port 9851 \
    --sequencer-rpc http://sequencer:8432 \
    --enable-dev-rpcs true \
    --enable-checkpoint-runner false \
    --reth-rpc http://reth:8545 $@
