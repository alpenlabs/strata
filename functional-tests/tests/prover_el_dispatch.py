import time

import flexitest

from envs import testenv
from utils import el_slot_to_block_id, wait_for_proof_with_time_out

# Parameters defining the range of Execution Engine (EE) blocks to be proven.
EE_PROVER_PARAMS = {
    "start_block": 1,
    "end_block": 3,
}


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        # Wait for the some block building
        time.sleep(5)

        # Dispatch the prover task
        start_block_id = el_slot_to_block_id(rethrpc, EE_PROVER_PARAMS["start_block"])
        end_block_id = el_slot_to_block_id(rethrpc, EE_PROVER_PARAMS["end_block"])

        task_ids = prover_client_rpc.dev_strata_proveElBlocks((start_block_id, end_block_id))
        self.debug(f"got task ids: {task_ids}")
        task_id = task_ids[0]
        self.debug(f"using task id: {task_id}")
        assert task_id is not None

        time_out = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
