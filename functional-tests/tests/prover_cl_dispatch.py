import time

import flexitest

from envs import testenv
from utils import cl_slot_to_block_id, wait_for_proof_with_time_out

# Parameters defining the range of Execution Engine (EE) blocks to be proven.
CL_PROVER_PARAMS = {
    "start_block": 1,
    "end_block": 2,
}


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        # Wait for the Prover Manager setup
        time.sleep(5)

        for i in range(30):
            print("\n\n Step: ", i)
            block_time = seqrpc.strata_blockTime()
            print("block_time ", block_time)

            time.sleep(10)

        # task_ids = prover_client_rpc.dev_strata_proveClBlocks((start_block_id, end_block_id))
        # task_id = task_ids[0]

        # self.debug(f"using task id: {task_id}")
        # assert task_id is not None

        # time_out = 10 * 60
        # wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
