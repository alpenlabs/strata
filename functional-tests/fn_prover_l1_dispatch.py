import time

import flexitest

import testenv
from utils import wait_for_proof_with_time_out

# Parameters defining the range of L1 blocks to be proven.
L1_SCAN_PROVER_PARAMS = {
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

        # Wait for the Prover Manager setup
        time.sleep(5)

        # Prove L1 blocks from START_BLOCK to END_BLOCK
        task_id = prover_client_rpc.dev_strata_proveBtcBlocks(
            (L1_SCAN_PROVER_PARAMS["start_block"], L1_SCAN_PROVER_PARAMS["end_block"])
        )
        print("got the task id: {}", task_id)
        assert task_id is not None

        time_out = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
