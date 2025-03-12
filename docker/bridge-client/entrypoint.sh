#!/bin/bash -e

export RUST_LOG=${RUST_LOG:-info}
export RUST_BACKTRACE=${RUST_BACKTRACE:-full}

export DATADIR=${DATADIR:-.data}
mkdir -p $DATADIR

KEYFILE=".secrets/xpriv.bin"

if [ ! -f "$KEYFILE" ]; then
    echo  "Error: Missing key file $KEYFILE";
    exit 1;
fi

XPRIV_STR=$(cat $KEYFILE | tr -d '\n')
RPC_HOST=${RPC_HOST:-127.0.0.1}
RPC_PORT=${RPC_PORT:-4781}

# delayed start to allow other containers to spin up first
# this is not enough for rollup genesis to be triggered
# so the associated container might have to be restarted upon first creation
# this delay only accounts for startup delays once genesis has happened
sleep 10

# Start the Strata Operator Client
strata-bridge-client operator \
  --master-xpriv $XPRIV_STR \
  --rpc-host $RPC_HOST \
  --rpc-port $RPC_PORT \
  --btc-url $BTC_URL \
  --btc-user $BTC_USER \
  --btc-pass $BTC_PASS \
  --rollup-url $ROLLUP_URL \
  --datadir $DATADIR $@
