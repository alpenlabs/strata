import flexitest

from envs import testenv
from utils import handle_bailout


@flexitest.register
class CrashAdvanceConsensusStateTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        """
        We encounter the following error after crash.

        strata_consensus_logic::duty::block_assembly: preparing block target_slot=7
        strata_consensus_logic::duty::block_assembly: was turn to propose block,
        but found block in database already slot=7 target_slot=7

        The check is present in the function `sign_and_store_block` on block_assembly.
        Further discussion is required regarding what to do here. which will re-enable this test
        """
        return
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        self.debug("checking connectivity")
        protocol_version = seqrpc.strata_protocolVersion()
        assert protocol_version is not None, "Sequencer RPC inactive"

        bail_context = "advance_consensus_state"
        handle_bailout(seq, seqrpc, bail_context)

        return True
