#!/bin/sh
# -----------------------------------------------------------------------------
# Script Name: generate_random_hex.sh
# Description:
#   - Creates a 'configs' directory if it doesn't exist.
#   - Generates a 64-character hexadecimal string (32 bytes) and saves it to
#     'configs/jwt.hex' if the file does not already exist.
# -----------------------------------------------------------------------------

# Function to generate a random hexadecimal string and save to a file
generate_random_hex() {
    local target_file="$1"

    # Ensure a file path is provided
    if [ -z "$target_file" ]; then
        echo "Error: No file path provided."
        return 1
    fi

    # Check if the file already exists to avoid overwriting
    if [ -e "$target_file" ]; then
        echo "File '$target_file' already exists. Skipping."
        return 0
    fi

    # Generate 32 random bytes, convert to hex, and save to the target file
    od -An -tx1 -N32 /dev/urandom | tr -d ' \n' > "$target_file"

    # Confirm successful creation
    if [ -e "$target_file" ]; then
        echo "Successfully generated random hex and saved to '$target_file'."
    else
        echo "Failed to generate the random hex file."
        return 1
    fi
}

# Create the 'configs' directory if it doesn't exist
mkdir -p configs

# Generate the random hex file within the 'configs' directory
generate_random_hex "configs/jwt.hex"