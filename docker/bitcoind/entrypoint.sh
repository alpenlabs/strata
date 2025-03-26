#! /bin/bash -x

# Generate bitcoin.conf
cat <<EOF > /root/.bitcoin/bitcoin.conf
regtest=1

[regtest]
rpcuser=${BITCOIND_RPC_USER}
rpcpassword=${BITCOIND_RPC_PASSWORD}
rpcbind=0.0.0.0
rpcallowip=${RPC_ALLOW_IP}
fallbackfee=0.00001
maxburnamount=1
server=1
txindex=1
acceptnonstdtxn=1
minrelaytxfee=0.0
blockmintxfee=0.0
dustRelayFee=0.0
EOF

echo "Bitcoin RPC User: $BITCOIND_RPC_USER"

bcli() {
    bitcoin-cli -regtest -rpcuser=${BITCOIND_RPC_USER} -rpcpassword=${BITCOIND_RPC_PASSWORD} $@
}

# Start bitcoind in the background
bitcoind -conf=/root/.bitcoin/bitcoin.conf -regtest $@ &

# Function to check if a wallet exists and is loaded, mainly for docker cache
check_wallet_exists() {
  echo "Checking if wallet '$1' exists in the wallet directory..."

  # List all wallets in the wallet directory
  ALL_WALLETS=$(bcli listwalletdir)

  echo $ALL_WALLETS

  # Check if the wallet name is in the list of wallets in the directory
  if echo "$ALL_WALLETS" | grep -q "\"name\": \"${1}\""; then
    echo "Wallet '$1' exists in the wallet directory."
    bcli loadwallet $1
  else
    echo "Wallet '$1' does not exist in the wallet directory."
    bcli -named createwallet wallet_name="${1}" descriptors=true
    bcli loadwallet $1
  fi

  return 0
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
check_wallet_exists $BITCOIND_WALLET
check_wallet_exists $BRIDGE_WALLET_1
check_wallet_exists $BRIDGE_WALLET_2
check_wallet_exists $BRIDGE_WALLET_3

VAL=$(bitcoin-cli getblockcount)

# Get a new Bitcoin address from the wallet
ADDRESS=$(bcli -rpcwallet="${BITCOIND_WALLET}" getnewaddress)

BRIDGE_ADDRESS_1=$(bcli -rpcwallet="${BRIDGE_WALLET_1}" getnewaddress)
BRIDGE_ADDRESS_2=$(bcli -rpcwallet="${BRIDGE_WALLET_2}" getnewaddress)
BRIDGE_ADDRESS_3=$(bcli -rpcwallet="${BRIDGE_WALLET_3}" getnewaddress)

echo "Generated new address: $ADDRESS"
echo $ADDRESS > /root/.bitcoin/bitcoin-address

# Generate 120 blocks to the new address
# (101 to mature the coinbase transactions and a few more for rollup genesis)
echo "Generating 120 blocks..."
bcli generatetoaddress 120 "$ADDRESS"

bcli generatetoaddress 101 "$BRIDGE_ADDRESS_1"
bcli generatetoaddress 101 "$BRIDGE_ADDRESS_2"
bcli generatetoaddress 101 "$BRIDGE_ADDRESS_3"

# generate single blocks
if [ ! -z $GENERATE_BLOCKS ];then
while :
do
    bcli generatetoaddress 1 "$ADDRESS"
    bcli generatetoaddress 1 "$BRIDGE_ADDRESS_1"
    bcli generatetoaddress 1 "$BRIDGE_ADDRESS_2"
    bcli generatetoaddress 1 "$BRIDGE_ADDRESS_3"
    sleep $GENERATE_BLOCKS
done
else
    wait -n
    exit $?
fi

