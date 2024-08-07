import time

from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest



NO_OF_BLOCKS_TO_RECEIVE = 3
BLOCK_NUMBER = 1

@flexitest.register
class ExecUpdateTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()
        time.sleep(2)
        recent_blks = seqrpc.alp_getRecentBlocks(NO_OF_BLOCKS_TO_RECEIVE)

        assert len(recent_blks) == NO_OF_BLOCKS_TO_RECEIVE

        print(recent_blks[BLOCK_NUMBER]["block_id"])
        exec_update  = seqrpc.alp_getExecUpdateById(recent_blks[1]["block_id"])
        assert exec_update["update_idx"] == BLOCK_NUMBER







