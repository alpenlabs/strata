import flexitest

import net_settings
from constants import (
    ERROR_PROOF_ALREADY_CREATED,
)
from entry import BasicEnvConfig, TestStrata
from utils import (
    check_nth_checkpoint_finalized,
    check_submit_proof_fails_for_nonexistent_batch,
)


@flexitest.register
class BlockFinalizationTest(TestStrata):
    """ """

    def __init__(self, ctx: flexitest.InitContext):
        premine_blocks = 101
        settings = net_settings.get_fast_batch_settings()
        settings.genesis_trigger = premine_blocks + 5

        # TODO apply the rest of these
        #    **FAST_BATCH_ROLLUP_PARAMS,
        #    # Setup reasonal horizon/genesis height
        #    "horizon_l1_height": premine_blocks - 3,
        #    "genesis_l1_height": premine_blocks + 5,
        #    "proof_publish_mode": {
        #        "timeout": self.timeout,

        ctx.set_env(BasicEnvConfig(premine_blocks, rollup_settings=settings))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Check for first 4 checkpoints
        for n in range(4):
            check_nth_checkpoint_finalized(n, seqrpc)
            self.debug(f"Pass checkpoint finalization for checkpoint {n}")

        check_already_sent_proof(seqrpc)


def check_already_sent_proof(seqrpc):
    try:
        # Proof for checkpoint 0 is already sent
        seqrpc.strataadmin_submitCheckpointProof(0, "abc123")
    except Exception as e:
        print(e)
        assert e.code == ERROR_PROOF_ALREADY_CREATED
    else:
        raise AssertionError("Expected rpc error")
