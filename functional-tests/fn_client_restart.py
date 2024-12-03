import logging
from pathlib import Path

import flexitest

import net_settings
from constants import (
    ERROR_PROOF_ALREADY_CREATED,
)
from entry import BasicEnvConfig
from utils import (
    check_nth_checkpoint_finalized,
    check_submit_proof_fails_for_nonexistent_batch,
    wait_until,
)


@flexitest.register
class BlockFinalizationSeqRestartTest(flexitest.Test):
    """This tests finalization when sequencer client restarts"""

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_settings=net_settings.get_fast_batch_settings()))
        self.logger = logging.getLogger(Path(__file__).stem)

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Check for first 2 checkpoints
        for n in range(2):
            check_nth_checkpoint_finalized(n, seqrpc)
            self.logger.debug(f"Pass checkpoint finalization for checkpoint {n}")

        # Stop sequencer
        seq.stop()

        # Now restart service
        seq.start()

        seqrpc = seq.create_rpc()
        wait_until(seqrpc.strata_protocolVersion, timeout=5)

        # Check for next 2 checkpoints
        for n in range(2, 4):
            check_nth_checkpoint_finalized(n, seqrpc)
            self.logger.debug(f"Pass checkpoint finalization for checkpoint {n}")

        check_already_sent_proof(seqrpc)


def check_already_sent_proof(seqrpc):
    try:
        # Proof for checkpoint 0 is already sent
        seqrpc.strataadmin_submitCheckpointProof(0, "abc123")
    except Exception as e:
        assert e.code == ERROR_PROOF_ALREADY_CREATED
    else:
        raise AssertionError("Expected rpc error")
