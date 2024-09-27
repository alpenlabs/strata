import flexitest

from constants import (
    ERROR_PROOF_ALREADY_CREATED,
    FAST_BATCH_ROLLUP_PARAMS,
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
        ctx.set_env(BasicEnvConfig(101, rollup_params=FAST_BATCH_ROLLUP_PARAMS))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Check for first 2 checkpoints
        for n in range(1, 3):
            check_nth_checkpoint_finalized(n, seqrpc)
            print(f"Pass checkpoint finalization for checkpoint {n}")

        # Stop sequencer
        seq.stop()

        # Now restart service
        seq.start()

        seqrpc = seq.create_rpc()
        wait_until(seqrpc.alp_protocolVersion, timeout=5)

        # Check for next 2 checkpoints
        for n in range(3, 5):
            check_nth_checkpoint_finalized(n, seqrpc)
            print(f"Pass checkpoint finalization for checkpoint {n}")

        check_already_sent_proof(seqrpc)


def check_already_sent_proof(seqrpc):
    try:
        # Proof for checkpoint 1 is already sent
        # TODO: fix this
        checkpoint_transition_hex = (
            "bb3d99b5b335e08ee93350cb99e493cd19d48d6bd003db7601b8c944e77394d52a26d41a9b958c704d158804a3432ff5"
            "c391b2c2ba2e0a8fb2892232c46bb81a750ef336fdd9458c1b543d4d4f84e25055a8cd9b9004776348cabf78b6561de4"
            "1ca021d172c6cf5d01e148d50c28fb9b6b7691d99b4b916dac6a86a4e06038a9947730d6a678d6ff08f7825122ecd829"
        )
        seqrpc.alpadmin_submitCheckpointProof(1, "abc123", checkpoint_transition_hex)
    except Exception as e:
        assert e.code == ERROR_PROOF_ALREADY_CREATED
    else:
        raise AssertionError("Expected rpc error")
