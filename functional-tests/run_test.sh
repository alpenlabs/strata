#! /bin/bash
set -e

source env.bash

if [ "$CARGO_RELEASE" = 1 ]; then
	export PATH=$(realpath ../target/release/):$PATH
else
	export PATH=$(realpath ../target/debug/):$PATH
fi

# Conditionally run cargo build based on PROVER_TEST
if [ "$PROVER_TEST" = 1 ]; then
    cargo build -F "prover" --release
else
    cargo build
fi


poetry run python entry.py $@

