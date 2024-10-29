import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import (
    ROLLUP_PARAMS_FOR_DEPOSIT_TX,
    SEQ_PUBLISH_BATCH_INTERVAL_SECS,
)
from utils import wait_for_proof_with_time_out, wait_until

EVM_WAIT_TIME = 2
SATS_TO_WEI = 10**10

withdrawal_intent_event_abi = {
    "anonymous": False,
    "inputs": [
        {"indexed": False, "internalType": "uint64", "name": "amount", "type": "uint64"},
        {"indexed": False, "internalType": "bytes", "name": "dest_pk", "type": "bytes32"},
    ],
    "name": "WithdrawalIntentEvent",
    "type": "event",
}
event_signature_text = "WithdrawalIntentEvent(uint64,bytes32)"


@flexitest.register
class BridgeDepositTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        evm_addr = "deedf001900dca3ebeefdeadf001900dca3ebeef"

        btc = ctx.get_service("bitcoin")
        btcrpc: BitcoindClient = btc.create_rpc()

        # Do deposit and collect the L1 and L2 where the deposit transaction was included
        l2_block_num, l1_deposit_txn_id = self.do_deposit(ctx, evm_addr)
        l1_deposit_txn_block_info = btcrpc.proxy.gettransaction(l1_deposit_txn_id)
        deposit_txn_block_num = l1_deposit_txn_block_info["blockheight"]

        # Init the prover client
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()
        time.sleep(60)

        # Dispatch the prover task
        # Proving task with with few L1 and L2 blocks including the deposit transaction
        l1_range = (deposit_txn_block_num - 1, deposit_txn_block_num + 1)
        l2_range = (l2_block_num - 1, l2_block_num + 1)
        task_id = prover_client_rpc.dev_strata_proveCheckpointRaw(0, l1_range, l2_range)
        print("got proving task_id ", task_id)
        assert task_id is not None

        time_out = 30 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)

    def do_deposit(self, ctx: flexitest.RunContext, evm_addr: str):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        amount_to_send = ROLLUP_PARAMS_FOR_DEPOSIT_TX["deposit_amount"] / 10**8
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

        btc_txn_id = btcrpc.sendrawtransaction(deposit_tx)["txid"]

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

        deposit_amount = ROLLUP_PARAMS_FOR_DEPOSIT_TX["deposit_amount"] * SATS_TO_WEI

        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        print(f"Balance after deposit: {balance}")

        net_balance = balance - original_balance
        assert net_balance == deposit_amount, f"invalid deposit amount: {net_balance}"

        wait_until(
            lambda: int(rethrpc.eth_blockNumber(), base=16) > current_block_num,
            error_with="not building blocks",
            timeout=EVM_WAIT_TIME * 2,
        )

        balance = int(rethrpc.eth_getBalance(f"0x{evm_addr}"), 16)
        net_balance = balance - original_balance
        assert (
            net_balance == deposit_amount
        ), f"deposit processed multiple times, extra: {balance - original_balance - deposit_amount}"

        # Scan the L2 blocks where the deposits were included
        start_block = 1
        end_block = int(rethrpc.eth_blockNumber(), base=16) + 1
        for block_num in range(start_block, end_block):
            block = rethrpc.eth_getBlockByNumber(hex(block_num), False)
            withdrawals = block.get("withdrawals", None)
            if withdrawals is not None and len(withdrawals) != 0:
                return block_num, btc_txn_id

        return None, btc_txn_id
