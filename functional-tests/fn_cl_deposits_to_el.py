import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import ROLLUP_BATCH_WITH_FUNDS, SEQ_PUBLISH_BATCH_INTERVAL_SECS
from entry import BasicEnvConfig


@flexitest.register
class L1ClientStatusTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_params=ROLLUP_BATCH_WITH_FUNDS))

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        addr = btcrpc.proxy.getnewaddress("", "bech32m")
        amount_to_send = ROLLUP_BATCH_WITH_FUNDS["deposit_amount"] / 10**8
        print(amount_to_send)
        name = ROLLUP_BATCH_WITH_FUNDS["rollup_name"].encode("utf-8").hex()
        print(name)
        evm_addr = "deadf001900dca3ebeefdeadf001900dca3ebeef"

        outputs = [{addr: amount_to_send}, {"data": f"{name}{evm_addr}"}]

        options = {"changePosition": 2}

        psbt_result = btcrpc.proxy.walletcreatefundedpsbt([], outputs, 0, options)
        psbt = psbt_result["psbt"]

        signed_psbt = btcrpc.proxy.walletprocesspsbt(psbt)

        finalized_psbt = btcrpc.proxy.finalizepsbt(signed_psbt["psbt"])
        deposit_tx = finalized_psbt["hex"]

        print(btcrpc.sendrawtransaction(deposit_tx))
        time.sleep(SEQ_PUBLISH_BATCH_INTERVAL_SECS)
        time.sleep(6)
        deposits = seqrpc.alp_getCurrentDeposits()
        print(deposits)

        assert len(deposits) > 0

        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()
        print(rethrpc.eth_blockNumber())
        print(rethrpc.eth_getBalance(f"0x{evm_addr}"))
