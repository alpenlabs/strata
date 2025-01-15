import time

import flexitest

from envs import testenv
from utils import el_slot_to_block_id, wait_for_proof_with_time_out_all

EE_PROVER_PARAMS = [
    {"start_block": 1, "end_block": 1},
    {"start_block": 2, "end_block": 2},
    {"start_block": 3, "end_block": 3},
    {"start_block": 4, "end_block": 4},
    {"start_block": 5, "end_block": 5},
]


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        time.sleep(30)
        time_out = 10 * 60
        task_ids = []

        start_time = time.time()

        el_prover_params = generate_ee_prover_params(start_block=1, end_block=13 * 1, step=13)
        print("Using el prover params: ", el_prover_params)

        for params in el_prover_params:
            start_block_id = el_slot_to_block_id(rethrpc, params["start_block"])
            end_block_id = el_slot_to_block_id(rethrpc, params["end_block"])
            task_id = prover_client_rpc.dev_strata_proveElBlocks((start_block_id, end_block_id))[0]
            task_ids.append(task_id)

        wait_for_proof_with_time_out_all(prover_client_rpc, task_ids, time_out)

        end_time = time.time()
        total_time = end_time - start_time
        print(f"Time taken: {total_time:.2f} seconds")


def generate_ee_prover_params(start_block: int, end_block: int, step: int = 1):
    return [
        {"start_block": block, "end_block": block}
        for block in range(start_block, end_block + 1, step)
    ]
