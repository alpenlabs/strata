import time

import flexitest

import testenv
from utils import wait_until


@flexitest.register
class RPCBailTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        self.debug("checking connectivity")
        protocol_version = seqrpc.strata_protocolVersion()
        assert protocol_version is not None, "Sequencer RPC inactive"

        # Bail out when there is SyncEvent message
        bail_context = "SyncEvent"
        handle_bailout(seq, seqrpc, bail_context)

        # Bail out when there is Fork choice message with new Block
        bail_context = "FcmNewBlock"
        handle_bailout(seq, seqrpc, bail_context)

        # Bail out when there is Fork choice message with new Block
        bail_context = "SignBlock"
        handle_bailout(seq, seqrpc, bail_context)


def handle_bailout(seq, seqrpc, bail_context):
    """
    Handles the bailout process for the given sequencer RPC.

    Args:
        seqrpc: The RPC interface for the sequencer.
        seq: The sequencer service instance.
        bail_context: The context in which to trigger the bailout.
        cur_chain_tip: The current chain tip slot to monitor progress.

    Raises:
        AssertionError: If the bailout or chain tip progress fails.
    """
    # wait for 2 seconds for chain tip slot to accumulate.
    # Since the chain tip requirement is not exact, we can sleep here
    time.sleep(2)
    cur_chain_tip = seqrpc.strata_clientStatus()["chain_tip_slot"]

    # Trigger the bailout
    seqrpc.stratadebug_bail(bail_context)

    # Ensure the sequencer bails out
    wait_until(
        lambda: not isinstance(seqrpc.strata_protocolVersion(), Exception),
        error_with="Sequencer didn't bail out",
    )

    # Stop the sequencer to update bookkeeping
    seq.stop()

    # Restart the sequencer
    seq.start()

    # Ensure the chain tip progresses
    wait_until(
        lambda: seqrpc.strata_clientStatus()["chain_tip_slot"] > cur_chain_tip,
        error_with="chain tip slot not progressing",
    )
