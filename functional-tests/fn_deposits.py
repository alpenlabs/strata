import time

from bitcoinlib.services.bitcoind import BitcoindClient
import flexitest


#TODO: update this after we have tools for reading interesting txns
# maybe write a deposit txn which gets identified by the cur deposits
@flexitest.register
class CheckDepositTest(flexitest.Test):
    NO_OF_BLOCKS_TO_RECEIVE = 3
    BLOCK_NUMBER = 2
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()
        time.sleep(2)
        recent_blks = seqrpc.alp_getRecentBlocks(self.NO_OF_BLOCKS_TO_RECEIVE)

        assert len(recent_blks) == self.NO_OF_BLOCKS_TO_RECEIVE

        current_deposits = seqrpc.alp_getCurrentDeposits()
        print(current_deposits)









