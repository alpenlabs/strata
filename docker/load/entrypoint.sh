#!/bin/bash

# Exit on error
set -e

echo "starting load service" && pwd

cd /app/functional-tests
source env.bash

poetry run python load_generation.py