import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from envs import testenv
from utils import bytes_to_big_endian, wait_for_proof_with_time_out

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

        start_block_hash = bytes_to_big_endian(
            btcrpc.proxy.getblockhash(L1_PROVER_PARAMS["START_BLOCK_HEIGHT"])
        )
        end_block_hash = bytes_to_big_endian(
            btcrpc.proxy.getblockhash(L1_PROVER_PARAMS["END_BLOCK_HEIGHT"])
        )

        task_ids = prover_client_rpc.dev_strata_proveL1Batch((start_block_hash, end_block_hash))
        task_id = task_ids[0]
        self.debug(f"Using task id: {task_id}")
        assert task_id is not None

        proof_timeout_seconds = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=proof_timeout_seconds)
