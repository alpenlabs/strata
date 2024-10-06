#!/bin/bash
# Usage: ./init-keys.sh <path_to_datatool_binary>
DATATOOL_PATH=${1:-./strata-datatool}

echo "Checking if 'base58' is installed.".
if ! command -v base58 &> /dev/null; then \
	echo "base58 not found. Please install with 'pip install base58'." \
	exit 1; \
fi

CONFIG_FILE=configs

JWT_FILE=$CONFIG_FILE/jwt.hex

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

generate_random_hex $JWT_FILE

SEQ_SEED_FILE=$CONFIG_FILE/sequencer.bin
OP1_SEED_FILE=$CONFIG_FILE/operator1.bin
OP2_SEED_FILE=$CONFIG_FILE/operator2.bin

$DATATOOL_PATH -b regtest genseed -f $SEQ_SEED_FILE
$DATATOOL_PATH -b regtest genseed -f $OP1_SEED_FILE
$DATATOOL_PATH -b regtest genseed -f $OP2_SEED_FILE

seqkey=$($DATATOOL_PATH -b regtest genseqpubkey -f ${SEQ_SEED_FILE})
op1pubkey=$($DATATOOL_PATH -b regtest genopxpub -f ${OP1_SEED_FILE})
op2pubkey=$($DATATOOL_PATH -b regtest genopxpub -f ${OP2_SEED_FILE})

ROLLUP_PARAMS_FILE=$CONFIG_FILE/params.json
$DATATOOL_PATH -b regtest genparams -n "alpenstrata" -s $seqkey -b $op1pubkey -b $op2pubkey --output $ROLLUP_PARAMS_FILE

echo "Decoding the privkey to hex-encoded private key"
# decode in base58 => hex-encode => reverse => get the first 64 chars (32 bytes) = reverse again for the original (removing new lines along the way)
SEQ_KEY_FILE=$CONFIG_FILE/sequencer.key.hex
cat $SEQ_SEED_FILE | base58 -dc | xxd -p | tr -d '\n' | rev | cut -c 1-64 | rev | tr -d '\n' | tee $SEQ_KEY_FILE
