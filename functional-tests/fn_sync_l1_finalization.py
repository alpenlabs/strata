import flexitest

from constants import (
    ERROR_PROOF_ALREADY_CREATED,
    FAST_BATCH_ROLLUP_PARAMS,
)
from entry import BasicEnvConfig
from utils import (
    check_for_nth_checkpoint_finalization,
    check_send_proof_for_non_existent_batch,
)


@flexitest.register
class BlockFinalizationTest(flexitest.Test):
    """ """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_params=FAST_BATCH_ROLLUP_PARAMS))

    def main(self, ctx: flexitest.RunContext):
        # FIXME: update and enable after proof is available
        return

        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        check_send_proof_for_non_existent_batch(seqrpc, 100)

        # Check for first 4 checkpoints
        for n in range(1, 5):
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
