#! /bin/bash

# Generate bitcoin.conf
cat <<EOF > /root/.bitcoin/bitcoin.conf
regtest=1

[regtest]
rpcuser=${BITCOIND_RPC_USER}
rpcpassword=${BITCOIND_RPC_PASSWORD}
rpcbind=0.0.0.0
rpcallowip=${RPC_ALLOW_IP}
fallbackfee=0.00001
server=1
txindex=1
EOF

echo "Bitcoin RPC User: $BITCOIND_RPC_USER"

bcli() {
    bitcoin-cli -regtest -rpcuser=${BITCOIND_RPC_USER} -rpcpassword=${BITCOIND_RPC_PASSWORD} $@
}

# Start bitcoind in the background
bitcoind -conf=/root/.bitcoin/bitcoin.conf -regtest $@ &

# Function to check if a wallet exists and is loaded, mainly for docker cache
check_wallet_exists() {
  echo "Checking if wallet '$BITCOIND_WALLET' exists in the wallet directory..."

  # List all wallets in the wallet directory
  ALL_WALLETS=$(bcli listwalletdir)

  echo $ALL_WALLETS

  # Check if the wallet name is in the list of wallets in the directory
  if echo "$ALL_WALLETS" | grep -q "\"name\": \"${BITCOIND_WALLET}\""; then
    echo "Wallet '$BITCOIND_WALLET' exists in the wallet directory."
    bcli loadwallet $BITCOIND_WALLET
    return 0  # Wallet exists
  else
    echo "Wallet '$BITCOIND_WALLET' does not exist in the wallet directory."
    return 1  # Wallet does not exist
  fi
}

# Function to check if bitcoind is ready
wait_for_bitcoind() {
  echo "Waiting for bitcoind to be ready..."
  for i in $(seq 1 10); do
    result=$(bcli getblockchaininfo 2>/dev/null)
    if [ $? -eq 0 ]; then
      echo "Bitcoind started"
      return 0
    else
      sleep 1
    fi
  done
  return 1
}

# Wait until bitcoind is fully started
wait_for_bitcoind

if [ $? -eq 1 ]; then
    echo "Bitcoin didn't start properly. Exiting"
    exit
fi

# create wallet
if ! check_wallet_exists; then
    bcli -named createwallet wallet_name="${BITCOIND_WALLET}" descriptors=true
fi

VAL=$(bitcoin-cli getblockcount)

if [[ $VAL -eq 0 ]]; then
    # Get a new Bitcoin address from the wallet
    ADDRESS=$(bcli getnewaddress)

    echo "Generated new address: $ADDRESS"
    echo $ADDRESS > /root/.bitcoin/bitcoin-address

    # Generate 120 blocks to the new address
    # (101 to mature the coinbase transactions and a few more for rollup genesis)
    echo "Generating 120 blocks..."
    bcli generatetoaddress 120 "$ADDRESS"
fi

wait -n

exit $?


