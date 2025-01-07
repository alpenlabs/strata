#! /bin/bash
set -e

source env.bash

if [ "$CARGO_RELEASE" = 1 ]; then
	export PATH=$(realpath ../target/release/):$PATH
else
	export PATH=$(realpath ../target/debug/):$PATH
fi

# Conditionally run cargo build based on PROVER_TEST
if [ ! -z $PROVER_TEST ]; then
    echo "Running on sp1-mock mode"
    cargo build --release -F sp1-mock
	export PATH=$(realpath ../target/release/):$PATH
else
    echo "Running on seq mode"
    cargo build
fi

poetry run python entry.py $@

