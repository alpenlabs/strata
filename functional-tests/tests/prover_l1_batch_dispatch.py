import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from envs import testenv
from utils import bytes_to_big_endian, wait_for_proof_with_time_out_all

# Parameters defining therange of L1 blocks to be proven.
L1_PROVER_PARAMS = {
    "START_BLOCK_HEIGHT": 1,
    "END_BLOCK_HEIGHT": 3,
}


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        prover_client = ctx.get_service("prover_client")

        btcrpc: BitcoindClient = btc.create_rpc()
        prover_client_rpc = prover_client.create_rpc()

        # Allow time for blocks to build
        time.sleep(5)

        task_ids = []
        start_time = time.time()
        btc_prover_params = generate_btc_prover_params(start_block=1, end_block=50, step=1)
        print("Using el prover params: ", btc_prover_params)

        for params in btc_prover_params:
            start_block_hash = bytes_to_big_endian(btcrpc.proxy.getblockhash(params["start_block"]))
            end_block_hash = bytes_to_big_endian(btcrpc.proxy.getblockhash(params["end_block"]))

            task_ids = prover_client_rpc.dev_strata_proveL1Batch((start_block_hash, end_block_hash))
            task_id = task_ids[0]
            self.debug(f"Using task id: {task_id}")
            assert task_id is not None
            task_ids.append(task_id)

        time_out = 10 * 60
        wait_for_proof_with_time_out_all(prover_client_rpc, task_ids, time_out)

        end_time = time.time()
        total_time = end_time - start_time
        print(f"Time taken: {total_time:.2f} seconds")


def generate_btc_prover_params(start_block: int, end_block: int, step: int = 1):
    return [
        {"start_block": block, "end_block": block}
        for block in range(start_block, end_block + 1, step)
    ]
