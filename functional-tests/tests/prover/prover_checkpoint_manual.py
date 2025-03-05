import time

import flexitest

from envs import testenv
from utils import wait_for_proof_with_time_out


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        self.warning("SKIPPING TEST prover_checkpoint_manual - not implemented")
        return True
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        # Wait until the prover client reports readiness
        wait_until(
            lambda: prover_client_rpc.dev_strata_getReport() is not None,
            error_with="Prover did not start on time",
        )

        # Test on with manual checkpoint
        checkpoint_idx = 1
        l1_range = (1, 5)
        l2_range = (1, 5)
        task_id = prover_client_rpc.dev_strata_proveCheckpointRaw(
            checkpoint_idx, l1_range, l2_range
        )
        self.debug(f"got the task id: {task_id}")
        assert task_id is not None

        time_out = 30
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
