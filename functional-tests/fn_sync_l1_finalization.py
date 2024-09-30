import flexitest

from constants import (
    ERROR_PROOF_ALREADY_CREATED,
    FAST_BATCH_ROLLUP_PARAMS,
)
from entry import BasicEnvConfig
from utils import (
    check_nth_checkpoint_finalized,
    check_submit_proof_fails_for_nonexistent_batch,
)


@flexitest.register
class BlockFinalizationTest(flexitest.Test):
    """ """

    def __init__(self, ctx: flexitest.InitContext):
        premine_blocks = 101
        rollup_params = {
            **FAST_BATCH_ROLLUP_PARAMS,
            # Setup reasonal horizon/genesis height
            "horizon_l1_height": premine_blocks - 3,
            "genesis_l1_height": premine_blocks + 5,
        }
        ctx.set_env(BasicEnvConfig(premine_blocks, rollup_params=rollup_params))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Check for first 4 checkpoints
        for n in range(1, 5):
            check_nth_checkpoint_finalized(n, seqrpc)
            print(f"Pass checkpoint finalization for checkpoint {n}")

        check_already_sent_proof(seqrpc)


def check_already_sent_proof(seqrpc):
    try:
        # Proof for checkpoint 1 is already sent
        seqrpc.alpadmin_submitCheckpointProof(1, "abc123")
    except Exception as e:
        print(e)
        assert e.code == ERROR_PROOF_ALREADY_CREATED
    else:
        raise AssertionError("Expected rpc error")
