#!/bin/sh

generate_random_hex() {
    if [ -z "$1" ]; then
        return 1
    fi

    if [ -e "$1" ]; then
        echo "File '$1' already exists. Skipping."
        return 0
    fi

    # Generate 32 random bytes, convert to hex, and write to the file
    od -An -tx1 -N32 /dev/urandom | tr -d ' \n' > "$1"
}

generate_random_hex "configs/jwt.hex"
generate_random_hex "configs/jwt.fn.hex"
generate_random_hex "configs/sequencer.key.hex"