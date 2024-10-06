#! /bin/bash

# Exit on error
set -e

echo "starting Prover client"

export RUST_LOG=trace

strata-prover-client \
    --rpc-port 9851\
    --sequencer-rpc http://sequencer:8432\
    --reth-rpc http://reth:8545 $@
