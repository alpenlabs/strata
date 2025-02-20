import flexitest

from mixins import seq_crash_mixin
from utils import wait_until
from envs import testenv


@flexitest.register
class CrashSyncEventFinalizeEpochTest(seq_crash_mixin.SeqCrashMixin):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(testenv.BasicEnvConfig(101))

    def main(self, ctx: flexitest.RunContext):
        cur_chain_tip = self.handle_bail(lambda: "sync_event_finalize_epoch", timeout=60)

        wait_until(
            lambda: self.seqrpc.strata_syncStatus()["tip_height"] > cur_chain_tip + 1,
            error_with="chain tip slot not progressing"
        )

        return True
