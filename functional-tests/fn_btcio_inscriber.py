import logging
import time
from pathlib import Path

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import SEQ_PUBLISH_BATCH_INTERVAL_SECS
from utils import generate_n_blocks, wait_until
from setup import TestStrata


@flexitest.register
class L1WriterTest(TestStrata):
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

        # Submit blob
        blobdata = "2c4253d512da5bb4223f10e8e6017ede69cc63d6e6126916f4b68a1830b7f805"
        _ = seqrpc.strataadmin_submitDABlob(blobdata)

        # Allow some time for sequencer to publish blob
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
