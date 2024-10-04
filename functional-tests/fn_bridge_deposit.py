import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import ROLLUP_PARAMS_FOR_DEPOSIT_TX, SEQ_PUBLISH_BATCH_INTERVAL_SECS
from entry import BasicEnvConfig
from utils import wait_until

EVM_WAIT_TIME = 2
SATS_TO_WEI = 10**10


@flexitest.register
class BridgeDepositTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_params=ROLLUP_PARAMS_FOR_DEPOSIT_TX))

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        print(ROLLUP_PARAMS_FOR_DEPOSIT_TX)

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        amount_to_send = ROLLUP_PARAMS_FOR_DEPOSIT_TX["deposit_amount"] / 10**8
        print(amount_to_send)
        name = ROLLUP_PARAMS_FOR_DEPOSIT_TX["rollup_name"].encode("utf-8").hex()
        print(name)
        evm_addr = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        # address from
        addr = "bcrt1pzupt5e8eqvt995r57jmmylxlswqfddsscrrq7njygrkhej3e7q2qur0c76"
        print(addr)
        outputs = [{addr: amount_to_send}, {"data": f"{name}{evm_addr}"}]

        options = {"changePosition": 2}

        psbt_result = btcrpc.proxy.walletcreatefundedpsbt([], outputs, 0, options)
        psbt = psbt_result["psbt"]

        signed_psbt = btcrpc.proxy.walletprocesspsbt(psbt)

        finalized_psbt = btcrpc.proxy.finalizepsbt(signed_psbt["psbt"])
        deposit_tx = finalized_psbt["hex"]

        print(btcrpc.sendrawtransaction(deposit_tx))
        # check if we are getting deposits
        wait_until(
            lambda: len(seqrpc.strata_getCurrentDeposits()) > 0,
            error_with="seem not be getting deposits",
            timeout=SEQ_PUBLISH_BATCH_INTERVAL_SECS,
        )

        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()
        print(rethrpc.eth_blockNumber())

        wait_until(
            lambda: int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16) > 0,
            error_with="zero eth balance",
            timeout=EVM_WAIT_TIME,
        )

        block_num = rethrpc.eth_blockNumber()
        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        assert (
            balance == ROLLUP_PARAMS_FOR_DEPOSIT_TX["deposit_amount"] * SATS_TO_WEI
        ), f"invalid deposit amount: {balance}"
        # sleep time > block production time
        time.sleep(2)
        block_num_after = rethrpc.eth_blockNumber()
        assert block_num_after > block_num, "not building blocks"
        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        assert (
            balance == ROLLUP_PARAMS_FOR_DEPOSIT_TX["deposit_amount"] * SATS_TO_WEI
        ), f"deposit processed multiple times: {balance}"
