import time

import flexitest

from utils import (
    wait_until,
)


@flexitest.register
class BlockFinalizationSeqRestartTest(flexitest.Test):
    """This tests finalization when sequencer client restarts"""

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("hub1")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("seq_node")
        full_node = ctx.get_service("follower_1_node")

        seqrpc = seq.create_rpc()
        noderpc = full_node.create_rpc()
        # wait until full sync with sequencer completely
        wait_until(
            lambda: noderpc.strata_syncStatus()["tip_height"]
            == seqrpc.strata_syncStatus()["tip_height"],
            error_with="seem to be not making progress",
            timeout=15,
        )

        blk_count = seqrpc.strata_syncStatus()["tip_height"]
        # stop noderpc so that sequencer has more blocks
        full_node.stop()
        time.sleep(1)

        # sequencer produces 5 more blocks while full node is stopped
        wait_until(
            lambda: seqrpc.strata_syncStatus()["tip_height"] > blk_count + 5,
            error_with="seem to be not making progress",
            timeout=10,
        )

        # restart full_node
        full_node.start()
        # now crash the sequencer
        time.sleep(1)
        seq.stop()

        # check if full node is getting l1 blocks, when seq is down
        new_height = 0
        for _ in range(0, 5):
            cur_height = noderpc.strata_l1status()["cur_height"]
            assert cur_height > new_height
            new_height = cur_height
            time.sleep(1)

        seq.start()
        time.sleep(1)

        # wait until nodes are close to sync upto two blocks behind
        wait_until(
            lambda: seqrpc.strata_syncStatus()["tip_height"]
            == noderpc.strata_syncStatus()["tip_height"],
            error_with="node sync lagging",
            timeout=15,
        )

        assert (
            seqrpc.strata_syncStatus()["finalized_block_id"]
            == seqrpc.strata_syncStatus()["finalized_block_id"]
        ), "block id mismatch"

        assert seqrpc.strata_getHeadersAtIdx(2) == noderpc.strata_getHeadersAtIdx(
            2
        ), "header mismatch"

        assert (
            seqrpc.strata_clientStatus()["buried_l1_height"]
            == noderpc.strata_clientStatus()["buried_l1_height"]
        ), "header mismatch"
