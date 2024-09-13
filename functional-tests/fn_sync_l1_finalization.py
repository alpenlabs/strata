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

        for n in range(4):
            check_for_nth_checkpoint_finalization(n, seqrpc)
            print(f"Pass checkpoint finalization for checkpoint {n}")



def check_for_nth_checkpoint_finalization(idx, seqrpc):
        syncstat = seqrpc.alp_syncStatus()
        checkpoint_info = seqrpc.alp_getCheckpointInfo(idx)
        print(f"checkpoint info for {idx}", checkpoint_info)

        assert (
            syncstat["finalized_block_id"] != checkpoint_info["l2_blockid"]
        ), "Checkpoint block should not yet finalize"

        checkpoint_info_1 = seqrpc.alp_getCheckpointInfo(idx + 1)

        assert checkpoint_info["idx"] == idx
        assert checkpoint_info_1 is None, f"There should be no checkpoint info for {idx + 1} index"

        to_finalize_blkid = checkpoint_info["l2_blockid"]

        # Post checkpoint proof
        seqrpc.alp_putCheckpointProof(idx, [1, 2, 3, 4])  ## TODO: hex

        # Wait till checkpoint finalizes, since our finalization depth is 4 and block generation time is 0.5s wait ~2 secs
        # Ideally this should be tested with controlled bitcoin block production
        time.sleep(4)

        syncstat = seqrpc.alp_syncStatus()
        print("Sync Stat", syncstat)
        assert to_finalize_blkid == syncstat["finalized_block_id"], "Block not finalized"
