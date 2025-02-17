import time
import logging

import flexitest

from envs import testenv
from factory import seqrpc
from utils import *


@flexitest.register
class SyncGenesisTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(testenv.BasicEnvConfig(101))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()

        time.sleep(1)

        def _check_genesis():
            try:
                # This should raise if we're before genesis.
                ss = seqrpc.strata_syncStatus()
                logging.debug(f"after genesis, tip is slot {ss['tip_height']} blkid {ss['tip_block_id']}")
                return True
            except seqrpc.RpcError as e:
                # This is the "before genesis" error code, meaning we're still
                # before genesis
                if e.code == -32607:
                    return False
                else:
                    raise e

        wait_until(_check_genesis, timeout=20, step=2)
        logging.info("checking that we're still making progress...")

        # Make sure we're making progress.
        stat = None
        last_slot = 0
        for _ in range(5):
            time.sleep(3)
            stat = seqrpc.strata_syncStatus()
            tip_slot = stat["tip_height"]
            tip_blkid = stat["tip_block_id"]
            cur_epoch = stat["cur_epoch"]
            logging.info(f"cur tip slot {tip_slot}, blkid {tip_blkid}, epoch {cur_epoch}")
            assert tip_slot >= last_slot, "cur slot went backwards"
            assert tip_slot > last_slot, "seem to not be making progress"
            last_slot = tip_slot
