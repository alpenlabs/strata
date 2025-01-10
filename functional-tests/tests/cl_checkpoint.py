import flexitest

from envs import net_settings, testenv
from utils import wait_until, wait_until_with_value

REORG_DEPTH = 3


@flexitest.register
class CLBlockWitnessDataGenerationTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        fast_batch_settings = net_settings.get_fast_batch_settings()
        ctx.set_env(
            testenv.BasicEnvConfig(pre_generate_blocks=101, rollup_settings=fast_batch_settings)
        )

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        # Wait for seq
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )

        ckp_idx = wait_until_with_value(
            lambda: seqrpc.strata_getLatestCheckpointIndex(),
            predicate=lambda idx: idx is not None,
            error_with="Checkpoint was not generated in time",
        )

        self.debug(f"checkpoint: {ckp_idx} found")

        ckp = seqrpc.strata_getCheckpointInfo(ckp_idx)
        assert ckp is not None

        # wait for checkpoint confirmation
        ckp_idx = wait_until_with_value(
            lambda: seqrpc.strata_getLatestCheckpointIndex(True),
            predicate=lambda v: v >= ckp_idx,
            error_with="Checkpoint was not finalized in time",
            timeout=60,
        )
        self.debug(f"checkpoint: {ckp_idx} finalized")

        ckp = seqrpc.strata_getCheckpointInfo(ckp_idx)
        # print(ckp)
        assert ckp is not None
        assert ckp["commitment"] is not None
