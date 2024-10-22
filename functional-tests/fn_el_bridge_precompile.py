import os

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from web3 import Web3
from web3._utils.events import get_event_data

from constants import (
    PRECOMPILE_BRIDGEOUT_ADDRESS,
    ROLLUP_PARAMS_FOR_DEPOSIT_TX,
    SEQ_PUBLISH_BATCH_INTERVAL_SECS,
)
from entry import BasicEnvConfig
from utils import wait_until

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
        ctx.set_env(BasicEnvConfig(101, rollup_params=ROLLUP_PARAMS_FOR_DEPOSIT_TX))

    def main(self, ctx: flexitest.RunContext):
        evm_addr = "deedf001900dca3ebeefdeadf001900dca3ebeef"
        self.do_deposit(ctx, evm_addr)
        self.do_withdrawal_precompile_call(ctx)

        # edge case where bridge out precompile address has balance
        evm_addr = PRECOMPILE_BRIDGEOUT_ADDRESS.lstrip("0x")
        self.do_deposit(ctx, evm_addr)

        block_num = self.do_withdrawal_precompile_call(ctx)
        print("got the block num ", block_num)

        import time

        time.sleep(60)

    def do_withdrawal_precompile_call(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")
        web3: Web3 = reth.create_web3()

        source = web3.address
        dest = web3.to_checksum_address(PRECOMPILE_BRIDGEOUT_ADDRESS)
        # 64 bytes
        dest_pk = os.urandom(32).hex()
        print("dest_pk", dest_pk)

        assert web3.is_connected(), "cannot connect to reth"

        original_block_no = web3.eth.block_number
        original_bridge_balance = web3.eth.get_balance(dest)
        original_source_balance = web3.eth.get_balance(source)

        # assert original_bridge_balance == 0

        # 10 rollup btc as wei
        to_transfer_wei = 10_000_000_000_000_000_000

        txid = web3.eth.send_transaction(
            {
                "to": dest,
                "value": hex(to_transfer_wei),
                "gas": hex(100000),
                "from": source,
                "data": dest_pk,
            }
        )

        receipt = web3.eth.wait_for_transaction_receipt(txid, timeout=5)

        assert receipt.status == 1, "precompile transaction failed"
        assert len(receipt.logs) == 1, "no logs or invalid logs"

        event_signature_hash = web3.keccak(text=event_signature_text).hex()
        log = receipt.logs[0]
        assert web3.to_checksum_address(log.address) == dest
        assert log.topics[0].hex() == event_signature_hash
        event_data = get_event_data(web3.codec, withdrawal_intent_event_abi, log)

        # 1 rollup btc = 10**18 wei
        to_transfer_sats = to_transfer_wei // 10_000_000_000

        assert event_data.args.amount == to_transfer_sats
        assert event_data.args.dest_pk.hex() == dest_pk

        final_block_no = web3.eth.block_number
        final_bridge_balance = web3.eth.get_balance(dest)
        final_source_balance = web3.eth.get_balance(source)

        assert original_block_no < final_block_no, "not building blocks"
        assert final_bridge_balance == original_bridge_balance, "bridge out funds not burned"
        total_gas_price = receipt.gasUsed * receipt.effectiveGasPrice
        assert (
            final_source_balance == original_source_balance - to_transfer_wei - total_gas_price
        ), "final balance incorrect"

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
