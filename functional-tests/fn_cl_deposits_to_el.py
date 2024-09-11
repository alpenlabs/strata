import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import ROLLUP_BATCH_WITH_FUNDS, SEQ_PUBLISH_BATCH_INTERVAL_SECS
from entry import BasicEnvConfig


@flexitest.register
class L1ClientStatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_params=ROLLUP_BATCH_WITH_FUNDS))

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        deposit_tx = "".join(
            [
                "020000000002e8030000000000002251209ec7be23a1ec17cd9c4",
                "b621d899eec02bacde1d754ab080f9e1ac8445820014e00000000",
                "0000000021" "6a",  # OP_RETURN
                "0a65787072657373737373",  # OP_PUSHBYTES_10 expresssss
                "140101010101010101010101010101010101010101",
                "00000000",  # OP_PUSHBYTES_20 "01"**20
            ]
        )

        print(deposit_tx)

        funded_tx = btcrpc.proxy.fundrawtransaction(deposit_tx)
        signed_tx = btcrpc.proxy.signrawtransactionwithwallet(funded_tx["hex"])

        print(btcrpc.sendrawtransaction(signed_tx["hex"]))

        time.sleep(SEQ_PUBLISH_BATCH_INTERVAL_SECS)
        deposits = seqrpc.alp_getCurrentDeposits()

        assert len(deposits) > 0
