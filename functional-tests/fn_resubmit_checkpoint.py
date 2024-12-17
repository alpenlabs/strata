from os import wait
from socket import timeout
import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import SEQ_PUBLISH_BATCH_INTERVAL_SECS
from utils import generate_n_blocks, wait_until, wait_until_with_value


@flexitest.register
class ResubmitCheckpointTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # generate 5 btc blocks
        generate_n_blocks(btcrpc, 5)

        # Generate some funds to sequencer
        seqaddr = seq.get_prop("address")

        # Wait for seq
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )

        verified_on = wait_until_with_value(
            lambda: seqrpc.strata_getL2BlockStatus(1),
            predicate=lambda val: isinstance(val,dict) and "Finalized" in val,
            error_with=f"transactions are not being Finalized",
            timeout=10
        )
        verified_block_hash = btcrpc.proxy.getblockhash(verified_on["Finalized"])
        block_data = btcrpc.getblock(verified_block_hash)
        envelope_data = ""
        for tx in block_data['txs']:
            try:
                envelope_data = get_envelope_pushdata(tx.witness_data().hex())
            except:
                continue

        # submit envelope data
        _ = seqrpc.strataadmin_submitDABlob(envelope_data)

        # Allow some time for sequencer to get the blob
        time.sleep(SEQ_PUBLISH_BATCH_INTERVAL_SECS)

        l1_status = seqrpc.strata_l1status()
        txid = l1_status["last_published_txid"]

        # Calculate scriptbpubkey for sequencer address
        addrdata = btcrpc.proxy.validateaddress(seqaddr)
        scriptpubkey = addrdata["scriptPubKey"]

        # Check if txn is present in mempool/blockchain and is spent to sequencer address
        tx = btcrpc.gettransaction(txid)

        # NOTE: could have just compared address
        # but bitcoinlib is somehow giving bc1* addr even though network is regtest
        assert (
            tx.outputs[0].lock_script.hex() == scriptpubkey
        ), "Output should be locked to sequencer's scriptpubkey"

        return True

def get_envelope_pushdata(inp: str):
    op_if = "63"
    op_endif = "68"
    op_pushbytes_33 = "21"
    op_false = "00"
    start_position = inp.index(f"{op_false}{op_if}")
    end_position = inp.index(f"{op_endif}{op_pushbytes_33}",start_position)
    op_if_block = inp[start_position+3:end_position]
    op_pushdata = "4d"
    pushdata_position = op_if_block.index(f"{op_pushdata}")
    # we don't want PUSHDATA + num bytes b401
    return op_if_block[pushdata_position+2+4:]




