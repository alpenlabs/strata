import flexitest

from envs import net_settings, testenv
from utils.utils import (
    check_already_sent_proof,
    check_nth_checkpoint_finalized,
    check_submit_proof_fails_for_nonexistent_batch,
)


@flexitest.register
class BlockFinalizationTimeoutTest(testenv.StrataTester):
    """
    This checks for finalization if proof is not submitted within a timeout period
    """

    def __init__(self, ctx: flexitest.InitContext):
        premine_blocks = 101
        self.proof_timeout = 5
        settings = net_settings.get_fast_batch_settings()
        settings.genesis_trigger = premine_blocks + 5
        settings.proof_timeout = self.proof_timeout

        ctx.set_env(testenv.BasicEnvConfig(premine_blocks, rollup_settings=settings))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Check for first 4 checkpoints
        for n in range(4):
            check_nth_checkpoint_finalized(n, seqrpc, None, proof_timeout=self.proof_timeout)
            self.debug(f"Pass checkpoint finalization for checkpoint {n}")

        # Proof for checkpoint 0 is already sent above
        check_already_sent_proof(seqrpc, 0)
