#!/bin/bash

RPCUSER=alpen
RPCPASSWORD=testnet
RPCPORT=18443

WALLET=alpen

bcli_start() {
    bitcoind -regtest -daemon --rpcuser=$RPCUSER --rpcpassword=$RPCPASSWORD --rpcport=$RPCPORT 
}

bcli() {
    bitcoin-cli -regtest --rpcuser=$RPCUSER --rpcpassword=$RPCPASSWORD --rpcport=$RPCPORT $@
}

kill_daemon() {
    echo killing daemon
    killall bitcoind
}

create_wallet() {
    bcli -named createwallet wallet_name=$1 load_on_startup=true
}

generate_address() {
    bcli -rpcwallet=$1 getnewaddress "bridge" "bech32"
}

check_bitcoind_rpc() {
  if bcli -getinfo > /dev/null 2>&1; then
    return 0
  else
    return 1
  fi
}

setup_wallet() {
    if bcli listwallets | grep -q "\"$1\"";then
      return 0
    else
      if bcli loadwallet $1 | grep -q "loaded successfully"; then
        return 0
      else
          create_wallet $1
          user_addr=$(generate_address $1)

          # load some btc to wallet
          bcli -rpcwallet=$1 generatetoaddress 100 $user_addr
          echo User address is $user_addr
      fi
    fi
}

#kill the already running bitcoind process
if pgrep -x "bitcoind" > /dev/null; then
    echo "Killing the already running bitcoind process"
    kill_daemon
    sleep 0.5 
fi


echo "starting bitcoin regnet"
bcli_start

# check if bitcoind rpc has started or not
while ! check_bitcoind_rpc; do
  echo "bcli hasn't started yet... retrying in 3 seconds"
  sleep 3
done

setup_wallet $WALLET

REGTEST_HOST=http://localhost REGTEST_PORT=$RPCPORT REGTEST_USERNAME=$RPCUSER REGTEST_PASSWORD=$RPCPASSWORD REGTEST_WALLET=$WALLET cargo test --package alpen-vertex-btcio --all-targets -- --nocapture








