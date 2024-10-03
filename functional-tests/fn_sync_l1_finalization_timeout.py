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
class BlockFinalizationTimeoutTest(flexitest.Test):
    """
    This checks for finalization if proof is not submitted within a timeout period
    """

    def __init__(self, ctx: flexitest.InitContext):
        premine_blocks = 101
        self.timeout = 5
        rollup_params = {
            **FAST_BATCH_ROLLUP_PARAMS,
            # Setup reasonal horizon/genesis height
            "horizon_l1_height": premine_blocks - 3,
            "genesis_l1_height": premine_blocks + 5,
            "proof_publish_mode": {
                "timeout": self.timeout,
            },
        }
        ctx.set_env(BasicEnvConfig(premine_blocks, rollup_params=rollup_params))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Check for first 4 checkpoints
        for n in range(4):
            check_nth_checkpoint_finalized(n, seqrpc, None, proof_timeout=self.timeout)
            print(f"Pass checkpoint finalization for checkpoint {n}")

        check_already_sent_proof(seqrpc)


def check_already_sent_proof(seqrpc):
    try:
        # Proof for checkpoint 0 is already sent
        seqrpc.alpadmin_submitCheckpointProof(0, "abc123")
    except Exception as e:
        assert e.code == ERROR_PROOF_ALREADY_CREATED
    else:
        raise AssertionError("Expected rpc error")
