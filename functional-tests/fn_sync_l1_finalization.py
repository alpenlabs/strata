import time

import flexitest


@flexitest.register
class BlockFinalizationTest(flexitest.Test):
    """ """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("fast_batches")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        time.sleep(2)

        syncstat = seqrpc.alp_syncStatus()
        idx = 0
        checkpoint_info = seqrpc.alp_getCheckpointInfo(idx)
        print(f"checkpoint info for {idx}", checkpoint_info)

        assert (
            syncstat["finalized_block_id"] != checkpoint_info["l2_blockid"]
        ), "Checkpoint block should not yet finalize"

        checkpoint_info_1 = seqrpc.alp_getCheckpointInfo(idx + 1)

        assert checkpoint_info["idx"] == idx
        assert checkpoint_info_1 is None, "There should be no checkpoint info for idx 1"

        to_finalize_blkid = checkpoint_info["l2_blockid"]

        # Post checkpoint proof
        seqrpc.alp_putCheckpointProof(idx, [1, 2, 3, 4])  ## TODO: hex

        # Polling interval is 5 secs for sequencer so sleep 1 sec extra
        time.sleep(6)

        # Wait till checkpoint finalizes, since our finalization depth is 4 and block generation time is 0.5s wait ~2 secs
        time.sleep(3)

        syncstat = seqrpc.alp_syncStatus()
        assert to_finalize_blkid == syncstat["finalized_block_id"], "Block not finalized"
