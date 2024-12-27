import flexitest

import net_settings
import testenv
from utils import (
    check_already_sent_proof,
    check_nth_checkpoint_finalized,
    check_submit_proof_fails_for_nonexistent_batch,
    wait_until,
)


@flexitest.register
class BlockFinalizationSeqRestartTest(testenv.StrataTester):
    """This tests finalization when sequencer client restarts"""

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(
            testenv.BasicEnvConfig(101, rollup_settings=net_settings.get_fast_batch_settings())
        )

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        prover = ctx.get_service("prover-client")
        prover_rpc = prover.create_rpc()

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Check for first 2 checkpoints
        for n in range(2):
            check_nth_checkpoint_finalized(n, seqrpc, prover_rpc)
            self.debug(f"Pass checkpoint finalization for checkpoint {n}")

        # Stop sequencer
        seq.stop()

        # Now restart service
        seq.start()

        seqrpc = seq.create_rpc()
        wait_until(seqrpc.strata_protocolVersion, timeout=5)

        # Check for next 2 checkpoints
        for n in range(2, 4):
            check_nth_checkpoint_finalized(n, seqrpc, prover_rpc)
            self.debug(f"Pass checkpoint finalization for checkpoint {n}")

        check_already_sent_proof(seqrpc, 0)
