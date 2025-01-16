import flexitest

from envs import net_settings, testenv
from utils import (
    check_already_sent_proof,
    check_nth_checkpoint_finalized,
    check_submit_proof_fails_for_nonexistent_batch,
    wait_until,
)


@flexitest.register
class BlockFinalizationTest(testenv.StrataTester):
    """ """

    def __init__(self, ctx: flexitest.InitContext):
        premine_blocks = 101
        settings = net_settings.get_fast_batch_settings()
        settings.genesis_trigger = premine_blocks + 5
        settings.proof_timeout = 5

        ctx.set_env(testenv.BasicEnvConfig(premine_blocks, rollup_settings=settings))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        prover = ctx.get_service("prover_client")
        prover_rpc = prover.create_rpc()

        # Wait for prover
        wait_until(
            lambda: prover_rpc.dev_strata_getReport() is not None,
            error_with="Prover did not start on time",
        )

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Check for first 4 checkpoints
        for n in range(4):
            check_nth_checkpoint_finalized(n, seqrpc, prover_rpc)
            self.debug(f"Pass checkpoint finalization for checkpoint {n}")

        # Proof for checkpoint 0 is already sent above
        check_already_sent_proof(seqrpc, 0)
