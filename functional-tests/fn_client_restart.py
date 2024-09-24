import flexitest

from constants import (
    ERROR_PROOF_ALREADY_CREATED,
    FAST_BATCH_ROLLUP_PARAMS,
)
from entry import BasicEnvConfig
from utils import (
    check_for_nth_checkpoint_finalization,
    check_send_proof_for_non_existent_batch,
    wait_until,
)


@flexitest.register
class BlockFinalizationSeqRestartTest(flexitest.Test):
    """This tests finalization when sequencer client restarts"""

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_params=FAST_BATCH_ROLLUP_PARAMS))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        check_send_proof_for_non_existent_batch(seqrpc, 100)

        # Check for first 2 checkpoints
        for n in range(1, 3):
            check_for_nth_checkpoint_finalization(n, seqrpc)
            print(f"Pass checkpoint finalization for checkpoint {n}")

        # Stop sequencer
        seq.stop()

        # Now restart service
        seq.start()

        seqrpc = seq.create_rpc()
        wait_until(seqrpc.alp_syncStatus, timeout=5)

        # Check for next 2 checkpoints
        for n in range(3, 5):
            check_for_nth_checkpoint_finalization(n, seqrpc)
            print(f"Pass checkpoint finalization for checkpoint {n}")

        check_already_sent_proof(seqrpc)


def check_already_sent_proof(seqrpc):
    try:
        # Proof for checkpoint 1 is already sent
        seqrpc.alp_submitCheckpointProof(1, "abc123")
    except Exception as e:
        assert e.code == ERROR_PROOF_ALREADY_CREATED
    else:
        raise AssertionError("Expected rpc error")
