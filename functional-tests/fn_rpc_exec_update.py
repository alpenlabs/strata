import time

from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest


@flexitest.register
class ExecUpdateTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()
        time.sleep(2)

        recent_blks = seqrpc.alp_getRecentBlocks(1)
        exec_update = seqrpc.alp_getExecUpdateById(recent_blks[0]["block_id"])
        print(exec_update)
        assert exec_update["update_idx"] == recent_blks[0]["block_idx"]
