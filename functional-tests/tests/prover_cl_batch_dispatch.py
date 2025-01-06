import time

import flexitest

from envs import testenv
from utils import wait_for_proof_with_time_out

# Parameters defining the aggeration of the CL blocks
CL_AGG_PARAMS = [
    {
        "start_block": 1,
        "end_block": 2,
    },
    {
        "start_block": 3,
        "end_block": 4,
    },
]


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        seq = ctx.get_service("sequencer")

        prover_client_rpc = prover_client.create_rpc()
        seqrpc = seq.create_rpc()

        # Wait for the Prover Manager setup
        time.sleep(5)

        batches = []
        for batch_info in CL_AGG_PARAMS:
            start_block_id = self.blockidx_2_blockid(seqrpc, batch_info["start_block"])
            end_block_id = self.blockidx_2_blockid(seqrpc, batch_info["end_block"])
            batches.append((start_block_id, end_block_id))

        task_ids = prover_client_rpc.dev_strata_proveL2Batch(batches)
        self.debug(f"got task ids: {task_ids}")
        task_id = task_ids[0]
        self.debug(f"using task id: {task_id}")
        assert task_id is not None

        time_out = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)

    def blockidx_2_blockid(self, seqrpc, blockidx):
        l2_blks = seqrpc.strata_getHeadersAtIdx(blockidx)
        return l2_blks[0]["block_id"]
