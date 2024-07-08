import time
import flexitest


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
        btc.generate_blocks(btcrpc, 0.05, 13)
        time.sleep(2)
        l1stat = seqrpc.alp_l1status()
        print("L1 status", l1stat)

        # invalidate three blocks
        recorded_11th_block = btc.block_list[11]
        btcrpc.proxy.invalidateblock(btc.block_list[10])
        time.sleep(1)

        # generate eight more blocks
        btc.generate_blocks(btcrpc, 0.05, 1)
        time.sleep(1)
        assert (
            recorded_11th_block != btc.block_list[11]
        ), "The 11th block was not invalidated, which means the reorg didn't happen"
        btc.generate_blocks(btcrpc, 0.05, 7)
        time.sleep(2)

        l1stat = seqrpc.alp_l1status()
        print("L1 status", l1stat)

        # check if current_height 18
        assert l1stat["cur_height"] == 18, "All Blocks were not read"
        assert l1stat["cur_tip_blkid"] == btc.block_list[17], "Block invalidation "
