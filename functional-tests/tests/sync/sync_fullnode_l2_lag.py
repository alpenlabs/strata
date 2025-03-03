import logging

import flexitest

from envs import testenv
from utils import *

FOLLOW_DIST = 1


@flexitest.register
class SyncFullNodeL2LagTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        env = testenv.HubNetworkEnvConfig(
            110, rollup_settings=RollupParamsSettings.new_default().fast_batch()
        )
        ctx.set_env(env)

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("seq_node")
        seqrpc = seq.create_rpc()
        fullnode = ctx.get_service("follower_1_node")
        fnrpc = fullnode.create_rpc()

        # Wait until sequencer and fullnode start
        wait_until(seqrpc.strata_protocolVersion, timeout=5)
        wait_until(fnrpc.strata_protocolVersion, timeout=5)

        # Pick a recent slot and make sure they're both the same.
        seqss = seqrpc.strata_syncStatus()
        check_slot = seqss["tip_height"] - FOLLOW_DIST
        check_both_at_same_slot(seqrpc, fnrpc, check_slot)

        # Now pause the sync worker so that we can have finalized epoch on L1,
        # but not corresponding block on L2 in full node
        paused = fnrpc.debug_pause_resume("SyncWorker", "PauseUntilResume")
        assert paused, "Should pause the fullnode sync worker"

        cur_epoch = seqss["cur_epoch"]

        # Wait until current epoch is finalized. By this time some blocks will
        # have been produced in l2 as well as the checkpoint will be posted to L1.
        wait_until_epoch_finalized(seqrpc, cur_epoch, timeout=10)

        # Full node tip after sync is paused
        fn_tip = fnrpc.strata_syncStatus()["tip_height"]

        # Get corresponding checkpoint block
        checkpt_info = seqrpc.strata_getCheckpointInfo(cur_epoch)
        checkpt_l1_blk_height = checkpt_info["l1_reference"]["block_height"]

        # Wait until full node's CSM reads upto checkpoint height - 1. At this
        # point it won't have fully processed L1 block at checkpoint height
        # because corresponding finalized l2 block is not present in db yet.
        wait_until_with_value(
            lambda: (
                fnrpc.strata_clientStatus()["tip_l1_block"]["height"],
                seqrpc.strata_clientStatus()["tip_l1_block"]["height"],
            ),
            lambda v: v[0] >= checkpt_l1_blk_height - 1,
            error_with="Fullnode L1 sync did not catch upto buried checkpoint height - 1",
            timeout=10,
            debug=True,
        )

        # FN tip after fn catches upto the buried checkpoint, should be the same as before
        new_fn_tip = fnrpc.strata_syncStatus()["tip_height"]
        assert fn_tip == new_fn_tip, "Fn tip should not progress while syncing is paused"
        seq_tip = seqrpc.strata_syncStatus()["tip_height"]
        assert new_fn_tip < seq_tip, "Fn tip should be less than sequencer tip"

        # Now resume
        resumed = fnrpc.debug_pause_resume("SyncWorker", "Resume")
        assert resumed, "Should resume the fullnode sync worker"

        # Now check the epoch finalization, it should finalize since full node has resumed l2 sync
        wait_until_with_value(
            lambda: (
                fnrpc.strata_clientStatus()["tip_l1_block"]["height"],
                seqrpc.strata_clientStatus()["tip_l1_block"]["height"],
            ),
            lambda v: v[0] >= checkpt_l1_blk_height,
            error_with="Fullnode L1 sync did not catch upto buried checkpoint",
            timeout=10,
            debug=True,
        )

        # Let's check that eventually the fullnode syncs with sequencer
        wait_until(
            fn_syncs_with_seq(fnrpc, seqrpc),
            error_with="Full node could not sync with sequencer",
            timeout=20,
        )


def fn_syncs_with_seq(fnrpc, seqrpc):
    def _f():
        fnss = fnrpc.strata_syncStatus()
        seqss = seqrpc.strata_syncStatus()
        seq_tip_slot = seqss["tip_height"]
        fn_tip_slot = fnss["tip_height"]

        logging.info(f"Seq tip slot {seq_tip_slot}, fn tip slot {fn_tip_slot}")
        return fn_tip_slot == seq_tip_slot

    return _f


def check_both_at_same_slot(seqrpc, fnrpc, check_slot):
    seq_headers = seqrpc.strata_getHeadersAtIdx(check_slot)
    assert len(seq_headers) > 0, f"seq node missing headers at slot {check_slot}"

    fn_headers = fnrpc.strata_getHeadersAtIdx(check_slot)
    assert len(fn_headers) > 0, f"follower node missing headers at slot {check_slot}"

    seq_hdr = seq_headers[0]
    fn_hdr = fn_headers[0]
    assert seq_hdr == fn_hdr, f"headers mismatched at slot {check_slot}"
