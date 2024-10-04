import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import MAX_HORIZON_POLL_INTERVAL_SECS
from entry import BasicEnvConfig
from utils import generate_n_blocks, wait_until


@flexitest.register
class L1StatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(auto_generate_blocks=False))

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()
        # generate 5 btc blocks
        generate_n_blocks(btcrpc, 5)

        # Wait for seq
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )

        received_block = btcrpc.getblock(btcrpc.proxy.getbestblockhash())
        l1stat = seqrpc.strata_l1status()

        # Time is in millis
        cur_time = l1stat["last_update"] // 1000

        # check if height on bitcoin is same as, it is seen in sequencer
        print(
            "L1 stat curr height:",
            l1stat["cur_height"],
            "Received from bitcoin:",
            received_block["height"],
        )
        assert (
            l1stat["cur_height"] == received_block["height"]
        ), "sequencer height doesn't match the bitcoin node height"

        # generate 2 more btc blocks
        generate_n_blocks(btcrpc, 2)
        time.sleep(MAX_HORIZON_POLL_INTERVAL_SECS * 2)

        next_l1stat = seqrpc.strata_l1status()
        elapsed_time = next_l1stat["last_update"] // 1000

        # check if L1 reader is seeing new L1 activity
        assert next_l1stat["cur_height"] - l1stat["cur_height"] == 2, "new blocks not read"
        assert elapsed_time > cur_time, "time not flowing properly"
