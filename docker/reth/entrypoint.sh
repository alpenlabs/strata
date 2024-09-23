#! /bin/bash

## Entry point for Dockerfile

# fail if env is missing
set -eu

mkdir -p /app/reth

# save the JWTSECRET to file
echo "$JWTSECRET" > jwt.hex

cat <<EOF > p2p.pem
-----BEGIN PRIVATE KEY-----
$P2P_SECRET_KEY
-----END PRIVATE KEY-----
" > p2p.bin
EOF

echo "starting Reth"

strata-reth \
    --disable-discovery \
    --datadir /app/reth \
    --authrpc.addr 0.0.0.0 \
    --authrpc.port $RETH_AUTH_RPC_PORT \
    --http \
    --http.addr 0.0.0.0 \
    --http.port $RETH_PORT \
    --authrpc.jwtsecret jwt.hex \
    --color never \
    --enable-witness-gen \
    --p2p-secret-key p2p.bin \
    -vvvv $@
