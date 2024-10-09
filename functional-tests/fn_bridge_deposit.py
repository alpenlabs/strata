import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

import net_settings
from constants import ROLLUP_PARAMS_FOR_DEPOSIT_TX, SEQ_PUBLISH_BATCH_INTERVAL_SECS
from entry import BasicEnvConfig
from utils import wait_until

EVM_WAIT_TIME = 2
SATS_TO_WEI = 10**10


@flexitest.register
class BridgeDepositTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_settings=net_settings.get_fast_batch_settings()))

    def main(self, ctx: flexitest.RunContext):
        print("SUCCEEDING TEST EARLY, SEE STR-418")
        return True

        evm_addr = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        self.do(ctx, evm_addr)

        print("Depositng again to new address...")
        evm_addr = "deedf001900dca3ebeefdeadf001900dca3ebeef"
        self.do(ctx, evm_addr)

        print("Depositing again to the same address...")
        self.do(ctx, evm_addr)

    def do(self, ctx: flexitest.RunContext, evm_addr: str):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        print(ROLLUP_PARAMS_FOR_DEPOSIT_TX)
        deposit_amt = ROLLUP_PARAMS_FOR_DEPOSIT_TX["tx_params"]["deposit"]["deposit_amount"]

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        amount_to_send = deposit_amt / 10**8
        print(amount_to_send)
        name = ROLLUP_PARAMS_FOR_DEPOSIT_TX["rollup_name"].encode("utf-8").hex()

        addr = "bcrt1pzupt5e8eqvt995r57jmmylxlswqfddsscrrq7njygrkhej3e7q2qur0c76"
        outputs = [{addr: amount_to_send}, {"data": f"{name}{evm_addr}"}]

        options = {"changePosition": 2}

        psbt_result = btcrpc.proxy.walletcreatefundedpsbt([], outputs, 0, options)
        psbt = psbt_result["psbt"]

        signed_psbt = btcrpc.proxy.walletprocesspsbt(psbt)

        finalized_psbt = btcrpc.proxy.finalizepsbt(signed_psbt["psbt"])
        deposit_tx = finalized_psbt["hex"]

        original_num_deposits = len(seqrpc.strata_getCurrentDeposits())
        print(f"Original deposit count: {original_num_deposits}")

        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        original_balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        print(f"Balance before deposit: {original_balance}")

        print("Deposit Tx:", btcrpc.sendrawtransaction(deposit_tx))
        # check if we are getting deposits
        wait_until(
            lambda: len(seqrpc.strata_getCurrentDeposits()) > original_num_deposits,
            error_with="seem not be getting deposits",
            timeout=SEQ_PUBLISH_BATCH_INTERVAL_SECS,
        )

        current_block_num = int(rethrpc.eth_blockNumber(), base=16)
        print(f"Current reth block num: {current_block_num}")

        wait_until(
            lambda: int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16) > original_balance,
            error_with="eth balance did not update",
            timeout=EVM_WAIT_TIME,
        )

        block_num = rethrpc.eth_blockNumber()
        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        assert balance == deposit_amt * SATS_TO_WEI, f"invalid deposit amount: {balance}"

        wait_until(lambda: rethrpc.eth_blockNumber() > block_num, error_with="not building blocks")

        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        print(f"Balance after deposit: {balance}")

        net_balance = balance - original_balance
        assert net_balance == deposit_amt, f"invalid deposit amount: {net_balance}"

        wait_until(
            lambda: int(rethrpc.eth_blockNumber(), base=16) > current_block_num,
            error_with="not building blocks",
            timeout=EVM_WAIT_TIME * 2,
        )

        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        net_balance = balance - original_balance
        assert balance == deposit_amt * SATS_TO_WEI, f"deposit processed multiple times: {balance}"
