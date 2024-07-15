import time
from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest


@flexitest.register
class L1ReadReorgTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        time.sleep(3)
        l1stat = seqrpc.alp_l1status()
        height_to_invalidate_from = int(l1stat["cur_height"]) - 3
        block_to_invalidate_from = btcrpc.proxy.getblockhash(height_to_invalidate_from)
        to_be_invalid_block = seqrpc.alp_l1blockHash(height_to_invalidate_from + 1)
        btcrpc.proxy.invalidateblock(block_to_invalidate_from)
        time.sleep(2)
        block_from_invalidated_height = seqrpc.alp_l1blockHash(
            height_to_invalidate_from + 1
        )
        assert (
            to_be_invalid_block != block_from_invalidated_height
        ), "The 3rd block was not invalidated, which means the reorg didn't happen"
