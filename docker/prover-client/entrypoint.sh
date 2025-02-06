#!/bin/bash

# Exit on error
set -e

echo "starting Prover client"

# Sample Entrypoint
strata-prover-client \
    --rpc-port 9851 \
    --enable-dev-rpcs true \
    --enable-checkpoint-runner false $@
