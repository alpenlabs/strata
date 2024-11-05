import os
from typing import List

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from web3 import Web3

from constants import (
    DEFAULT_ROLLUP_PARAMS,
    PRECOMPILE_BRIDGEOUT_ADDRESS,
    ROLLUP_PARAMS_FOR_DEPOSIT_TX,
)
from entry import BasicEnvConfig
from utils import get_logger, wait_until


@flexitest.register
class BridgeDepositTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101, rollup_params=ROLLUP_PARAMS_FOR_DEPOSIT_TX))
        self.logger = get_logger("getBridgeDuties")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        reth = ctx.get_service("reth")
        web3: Web3 = reth.create_web3()

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        operator_idx = 0
        start_index = 0

        num_withdrawals = 5

        # make deposit utxo available for withthdrawals to use
        # FIXME: use deposit request instead
        self.assign_mock_deposits(web3, btcrpc, num_withdrawals)

        withdrawal_pubkeys = []
        last_txid = None
        for _ in range(num_withdrawals):
            pubkey = os.urandom(32).hex()
            last_txid = self.do_withdrawal(web3, pubkey)
            withdrawal_pubkeys.append(pubkey)

        web3.eth.wait_for_transaction_receipt(last_txid, timeout=5)

        # NOTE: this test might be flaky if new checkpoint is already generated at this point
        duties_resp = seqrpc.strata_getBridgeDuties(operator_idx, start_index)
        assert (
            len(duties_resp["duties"]) == 0
        ), "no duties should be generated before checkpoint creation"

        # wait for checkpoint
        prev_checkpoint_idx = int(seqrpc.strata_getLatestCheckpointIndex())
        wait_until(
            lambda: int(seqrpc.strata_getLatestCheckpointIndex()) > prev_checkpoint_idx,
            error_with="Checkpoint not posted in time",
            timeout=10,
        )
        # checkpoint with withdrawals is created but not finalized
        duties_resp = seqrpc.strata_getBridgeDuties(operator_idx, start_index)
        assert (
            len(duties_resp["duties"]) == 0
        ), "no duties should be generated before checkpoint finalization"
        wait_until(
            lambda: int(seqrpc.strata_getLatestCheckpointIndex()) > prev_checkpoint_idx + 1,
            error_with="Checkpoint not posted in time",
            timeout=10,
        )
        # checkpoint with withdrawals is finalized
        duties_resp = seqrpc.strata_getBridgeDuties(operator_idx, start_index)
        assert (
            len(duties_resp["duties"]) == num_withdrawals
        ), "duties should be generated after checkpoint finalization"

    def assign_mock_deposits(self, web3: Web3, btcrpc: BitcoindClient, count: int):
        addr = "bcrt1pzupt5e8eqvt995r57jmmylxlswqfddsscrrq7njygrkhej3e7q2qur0c76"
        sats_per_btc = 10**8
        amount_to_send = DEFAULT_ROLLUP_PARAMS["deposit_amount"] / sats_per_btc

        el_address = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        magic_bytes = DEFAULT_ROLLUP_PARAMS["rollup_name"].encode("utf-8").hex()
        outputs = [
            {addr: amount_to_send},
            {"data": f"{magic_bytes}{el_address}"},
        ]

        options = {"changePosition": 2}

        for _ in range(count):
            self.broadcast_tx(btcrpc, outputs, options)

        wei_per_sat = 10_000_000_000

        expected_deposit_wei = DEFAULT_ROLLUP_PARAMS["deposit_amount"] * wei_per_sat * count
        wait_until(
            lambda: web3.eth.get_balance(web3.to_checksum_address(el_address))
            >= expected_deposit_wei,
            timeout=10,
        )

    def do_withdrawal(self, web3: Web3, dest_pk: str) -> str:
        source = web3.address
        dest = web3.to_checksum_address(PRECOMPILE_BRIDGEOUT_ADDRESS)

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
        # print("txid", txid.to_0x_hex())
        # receipt = web3.eth.wait_for_transaction_receipt(txid, timeout=5)
        # assert receipt.status == 1, "precompile transaction failed"
        return txid

    def broadcast_tx(self, btcrpc: BitcoindClient, outputs: List[dict], options: dict) -> str:
        psbt_result = btcrpc.proxy.walletcreatefundedpsbt([], outputs, 0, options)
        psbt = psbt_result["psbt"]

        signed_psbt = btcrpc.proxy.walletprocesspsbt(psbt)

        finalized_psbt = btcrpc.proxy.finalizepsbt(signed_psbt["psbt"])
        deposit_tx = finalized_psbt["hex"]

        txid = btcrpc.sendrawtransaction(deposit_tx).get("txid", "")

        return txid
