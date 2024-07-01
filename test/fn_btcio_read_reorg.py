import time
import flexitest
from block_generator import generate_blocks, block_list


@flexitest.register
class L1ReadReorgTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        btcrpc = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # add 13 blocks
        generate_blocks(btcrpc, 0.05, 13)
        time.sleep(2)
        l1stat = seqrpc.alp_l1status()
        print("L1 status", l1stat)

        # invalidate three blocks
        btcrpc.proxy.invalidateblock(block_list[10])
        time.sleep(1)

        # generate eight more blocks
        generate_blocks(btcrpc, 0.05, 8)
        time.sleep(1)
        l1stat = seqrpc.alp_l1status()
        print("L1 status", l1stat)

        # check if current_height 18
        assert l1stat["cur_height"] == 18, "All Blocks were not read"
        # total 21 blocks were read, but we invalidated 3 blocks so current_blkid should match 21st block
        assert l1stat["cur_tip_blkid"] == block_list[20], "Block invalidation "
