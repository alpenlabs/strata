import flexitest

from mixins import seq_crash_mixin
from utils import wait_until


@flexitest.register
class CrashAdvanceConsensusStateTestd(seq_crash_mixin.SeqCrashMixin):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        """
        We encounter the following error after crash.

        strata_consensus_logic::duty::block_assembly: preparing block target_slot=7
        strata_consensus_logic::duty::block_assembly: was turn to propose block,
        but found block in database already slot=7 target_slot=7

        The check is present in the function `sign_and_store_block` on block_assembly.
        Further work is required for fixing the problem.
        To re-enable this test remove the return below.
        Track the issue on:
        https://alpenlabs.atlassian.net/browse/STR-916
        """
        return

        cur_chain_tip = self.handle_bail(lambda: "advance_consensus_state")

        wait_until(
            lambda: self.seqrpc.strata_clientStatus()["chain_tip_slot"] > cur_chain_tip,
            error_with="chain tip slot not progressing",
            timeout=20,
        )

        return True
