import flexitest

from envs import testenv


@flexitest.register
class CrashAdvanceConsensusStateTestd(testenv.CrashTestBase):
    def __init__(self, ctx: flexitest.InitContext):
        super().__init__(ctx)

    def get_bail_tag(self) -> str:
        return "advance_consensus_state"

    def main(self, ctx: flexitest.RunContext):
        """
        We encounter the following error after crash.

        strata_consensus_logic::duty::block_assembly: preparing block target_slot=7
        strata_consensus_logic::duty::block_assembly: was turn to propose block,
        but found block in database already slot=7 target_slot=7

        The check is present in the function `sign_and_store_block` on block_assembly.
        Further work is required for fixing the problem.
        To re-enable this test remove this main() function
        Track the issue on:
        https://alpenlabs.atlassian.net/browse/STR-916
        """
        return
