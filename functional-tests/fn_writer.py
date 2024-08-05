import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from bitcoinlib.encoding import addr_bech32_to_pubkeyhash, addr_base58_to_pubkeyhash
from bitcoinlib.transactions import Script

from constants import MAX_HORIZON_POLL_INTERVAL_SECS, SEQ_SLACK_TIME_SECS, SEQ_PUBLISH_BATCH_INTERVAL_SECS


@flexitest.register
class L1WriterTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # Generate some funds to sequencer
        seqaddr = seq.props["address"]
        print("Generating to address", seqaddr);
        print(f"First generating 101 blocks for initial usable fund")
        btcrpc.proxy.generatetoaddress(101, seqaddr)

        l1_status = seqrpc.alp_l1status()
        assert l1_status["last_published_txid"] is None, "Unexpected last_published_txid"

        time.sleep(MAX_HORIZON_POLL_INTERVAL_SECS + SEQ_SLACK_TIME_SECS)

        # Submit blob
        blobdata = "2c4253d512da5bb4223f10e8e6017ede69cc63d6e6126916f4b68a1830b7f805"
        _ = seqrpc.alpadmin_submitDABlob(blobdata)

        # Allow some time for sequencer to publish blob
        time.sleep(SEQ_PUBLISH_BATCH_INTERVAL_SECS)

        l1_status = seqrpc.alp_l1status()
        txid = l1_status["last_published_txid"]

        # Calculate scriptbpubkey for sequencer address
        addrdata = btcrpc.proxy.validateaddress(seqaddr)
        scriptpubkey = addrdata["scriptPubKey"]

        # Check if txn is present in mempool/blockchain and is spent to sequencer address
        tx = btcrpc.gettransaction(txid)

        # NOTE: could have just compared address, but bitcoinlib is somehow giving bc1* addr even though network is regtest
        assert tx.outputs[0].lock_script.hex() == scriptpubkey, "Output should be locked to sequencer's scriptpubkey"

        return True
