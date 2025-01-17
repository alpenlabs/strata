import flexitest

from envs import testenv
from utils import handle_bailout


@flexitest.register
class CrashDutySignBlockTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        self.debug("checking connectivity")
        protocol_version = seqrpc.strata_protocolVersion()
        assert protocol_version is not None, "Sequencer RPC inactive"

        bail_context = "duty_sign_block"
        handle_bailout(seq, seqrpc, bail_context)

        return True
