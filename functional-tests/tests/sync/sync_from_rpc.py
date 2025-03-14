import logging
import time

import flexitest

from envs import testenv
from utils.utils import wait_until_epoch_confirmed

FOLLOW_DIST = 1


@flexitest.register
class SyncFromRpcTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("hub1")

    def main(self, ctx: flexitest.RunContext):
        seqrpc = ctx.get_service("seq_node").create_rpc()
        btcrpc = ctx.get_service("bitcoin").create_rpc()
        fnrpc = ctx.get_service("follower_1_node").create_rpc()
        seq_reth_rpc = ctx.get_service("seq_reth").create_rpc()
        fullnode_reth_rpc = ctx.get_service("follower_1_reth").create_rpc()

        # give some time for the sequencer to start up and generate blocks
        time.sleep(5)

        # Pick a recent slot and make sure they're both the same.
        seqss = seqrpc.strata_syncStatus()
        seq_tip_slot = seqss["tip_height"]
        check_slot = seq_tip_slot - FOLLOW_DIST

        seq_headers = seqrpc.strata_getHeadersAtIdx(check_slot)
        logging.info(f"sequencer sees {seq_headers}")
        assert len(seq_headers) > 0, f"seq node missing headers at slot {check_slot}"

        fn_headers = fnrpc.strata_getHeadersAtIdx(check_slot)
        logging.info(f"fn sees {fn_headers}")
        assert len(fn_headers) > 0, f"follower node missing headers at slot {check_slot}"

        seq_hdr = seq_headers[0]
        fn_hdr = fn_headers[0]
        assert seq_hdr == fn_hdr, f"headers mismatched at slot {check_slot}"

        # Now *also* check the reth nodes.
        last_blocknum = int(seq_reth_rpc.eth_blockNumber(), 16)

        time.sleep(3)

        # test an older block because latest may not have been synced yet
        test_blocknum = last_blocknum - 1

        assert test_blocknum > 0, "not enough blocks generated"

        block_from_sequencer = seq_reth_rpc.eth_getBlockByNumber(hex(test_blocknum), False)
        assert block_from_sequencer, "sequencer EL client missing block"
        seq_el_hash = block_from_sequencer["hash"]

        block_from_fullnode = fullnode_reth_rpc.eth_getBlockByNumber(hex(test_blocknum), False)
        assert block_from_fullnode, "follower EL client missing block"
        fn_el_hash = block_from_fullnode["hash"]

        logging.info(
            f"block at height {test_blocknum},\n \
            \tseq {block_from_sequencer},\n\tfn {block_from_fullnode}"
        )
        assert seq_el_hash == fn_el_hash, "EL blocks don't match"

        # Check fullnode sees same checkpoint reference as sequencer
        epoch = 1
        wait_until_epoch_confirmed(seqrpc, epoch)

        fn_checkpt_info = fnrpc.strata_getCheckpointInfo(epoch)
        sq_checkpt_info = seqrpc.strata_getCheckpointInfo(epoch)

        assert fn_checkpt_info["l1_reference"] == sq_checkpt_info["l1_reference"]
        assert fn_checkpt_info["confirmation_status"] == sq_checkpt_info["confirmation_status"]

        # Check l1_reference txid and blockids are actually present in bitcoin
        txid = fn_checkpt_info["l1_reference"]["txid"]
        txdata = btcrpc.proxy.gettransaction(txid)
        assert txdata["confirmations"] > 0

        blkid = fn_checkpt_info["l1_reference"]["block_id"]
        blkheight = fn_checkpt_info["l1_reference"]["block_height"]
        blkdata = btcrpc.proxy.getblock(blkid)
        assert blkdata["confirmations"] > 0
        assert blkheight > 0
