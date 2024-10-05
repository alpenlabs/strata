#! /bin/bash

# Exit on error
set -e

echo "starting Prover client"

strata-prover-client \
    --rpc-port 9851\
    --sequencer-rpc sequencer:8432\
    --reth-rpc reth:8545 $@

