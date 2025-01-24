import flexitest
from web3 import Web3

from envs import testenv


class BaseMixin(testenv.StrataTester):
    def premain(self, ctx: flexitest.RunContext):
        super().premain(ctx)
        self._ctx = ctx

        self.btc = ctx.get_service("bitcoin")
        self.seq = ctx.get_service("sequencer")
        self.reth = ctx.get_service("reth")

        self.seqrpc = self.seq.create_rpc()
        self.btcrpc = self.btc.create_rpc()
        self.rethrpc = self.reth.create_rpc()
        self.web3: Web3 = self.reth.create_web3()
