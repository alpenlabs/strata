#! /bin/bash

source env.bash 

if [ "$CARGO_RELEASE" = 1 ]; then
	export PATH=$(realpath ../target/release/):$PATH
else
	export PATH=$(realpath ../target/debug/):$PATH
fi

cargo build

poetry run python entry.py $@

