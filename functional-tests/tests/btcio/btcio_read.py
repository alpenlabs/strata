import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from envs import testenv
from utils import generate_n_blocks, wait_until_with_value


@flexitest.register
class L1StatusTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(testenv.BasicEnvConfig(auto_generate_blocks=False))

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        # create both btc and sequencer RPC
        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()
        # generate 5 btc blocks
        generate_n_blocks(btcrpc, 5)

        received_block = wait_until_with_value(
            lambda: btcrpc.getblock(btcrpc.proxy.getbestblockhash()),
            predicate=lambda block: block["height"] == 5,
            error_with="btc blocks not generated",
        )

        # Wait for seq to catch up
        l1stat = wait_until_with_value(
            lambda: seqrpc.strata_l1status(),
            predicate=lambda stat: stat["cur_height"] == 5,
            error_with="Sequencer did not catch up",
        )

        # Time is in millis
        cur_time = l1stat["last_update"]

        # check if height on bitcoin is same as, it is seen in sequencer
        self.debug(f"L1 stat curr height: {l1stat['cur_height']}")
        self.debug(f"Received from bitcoin: {received_block['height']}")
        assert l1stat["cur_tip_blkid"] == received_block["block_hash"], (
            "sequencer block doesn't match the bitcoin node block"
        )

        # generate 2 more btc blocks
        generate_n_blocks(btcrpc, 2)

        received_block = wait_until_with_value(
            lambda: btcrpc.getblock(btcrpc.proxy.getbestblockhash()),
            predicate=lambda block: block["height"] == 7,
            error_with="btc blocks not generated",
        )

        # Wait for seq to catch up
        next_l1stat = wait_until_with_value(
            lambda: seqrpc.strata_l1status(),
            predicate=lambda stat: stat["cur_height"] == 7,
            error_with="Sequencer did not catch up",
        )

        elapsed_time = next_l1stat["last_update"]

        # check if L1 reader is seeing new L1 activity
        assert next_l1stat["cur_tip_blkid"] == received_block["block_hash"], (
            "sequencer block doesn't match the bitcoin node block"
        )
        assert elapsed_time > cur_time, "time not flowing properly"
