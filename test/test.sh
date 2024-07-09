#! /bin/bash

pushd .. > /dev/null
PATH=$(pwd)/target/debug/:$PATH
export RUST_LOG=info

cd test
python3 entry.py
popd .. > /dev/null

