#! /bin/bash

## Entry point for Dockerfile

# fail if env is missing
set -eu

mkdir /app/reth

# For now create a sample jwtsecret
echo "7df2f4bc998cbf0631d0c871bf06b5caa3f2ab48afa1856b81b814afa898bf71" > jwt.hex

alpen-express-reth
--disable-discovery \
--datadir /app/reth \
--authrpc.port $RETH_PORT \
--authrpc.jwtsecret jwt.hex \
--port $LISTENER_PORT \
--ws
--ws.port $WEBSOCKET_PORT \
--http
--http.port $HTTP_PORT \
--color never \
--enable-witness-gen
-vvvv
