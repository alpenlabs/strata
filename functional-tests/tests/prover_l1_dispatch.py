import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from envs import testenv
from utils import bytes_to_big_endian, wait_for_proof_with_time_out


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        prover_client = ctx.get_service("prover_client")

        btcrpc: BitcoindClient = btc.create_rpc()
        prover_client_rpc = prover_client.create_rpc()

        # Wait for the Prover Manager setup
        time.sleep(5)

        # Dispatch the prover task
        block_height = 1
        blockhash = btcrpc.proxy.getblockhash(block_height)
        print(block_height, blockhash)

        task_ids = prover_client_rpc.dev_strata_proveBtcBlock(bytes_to_big_endian(blockhash))
        self.debug(f"got task ids: {task_ids}")
        task_id = task_ids[0]
        self.debug(f"using task id: {task_id}")
        assert task_id is not None

        time_out = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
