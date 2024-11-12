import time
from typing import List

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import DEFAULT_ROLLUP_PARAMS, SEQ_PUBLISH_BATCH_INTERVAL_SECS
from entry import BasicEnvConfig
from utils import broadcast_tx, get_logger
from strata_utils import get_address


@flexitest.register
class BridgeDutiesTest(flexitest.Test):
    """
    Test that the bridge client can fetch bridge duties correctly.
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101))
        self.logger = get_logger("getBridgeDuties")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        addr = get_address(0)
        fees_in_btc = 0.01
        sats_per_btc = 10**8
        amount_to_send = DEFAULT_ROLLUP_PARAMS["deposit_amount"] / sats_per_btc + fees_in_btc

        el_address = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        take_back_leaf_hash = "02" * 32
        magic_bytes = DEFAULT_ROLLUP_PARAMS["rollup_name"].encode("utf-8").hex()
        outputs = [
            {addr: amount_to_send},
            {"data": f"{magic_bytes}{take_back_leaf_hash}{el_address}"},
        ]

        options = {"changePosition": 2}

        num_blocks = 5
        num_deposits_per_block = 2

        txids = []
        for i in range(num_blocks):
            for j in range(num_deposits_per_block):
                txid = broadcast_tx(btcrpc, outputs, options)
                txids.append(txid)

                # add robustness by spreading out requests across blocks
                self.logger.debug(f"sent deposit request #{j} with txid = {txid} to block #{i}")

        time.sleep(SEQ_PUBLISH_BATCH_INTERVAL_SECS)

        operator_idx = 0
        start_index = 0
        self.logger.debug(
            f"getting bridge duties for operator_idx: {operator_idx} from index: {start_index}"
        )
        duties_resp = seqrpc.strata_getBridgeDuties(operator_idx, start_index)
        duties: List = duties_resp["duties"]

        expected_duties = []
        for txid in txids:
            expected_duty = {
                "type": "SignDeposit",
                "payload": {
                    "deposit_request_outpoint": f"{txid}:0",
                    "el_address": list(bytes.fromhex(el_address)),
                    "total_amount": amount_to_send * sats_per_btc,
                    "take_back_leaf_hash": take_back_leaf_hash,
                    "original_taproot_addr": {"network": "regtest", "address": addr},
                },
            }
            expected_duties.append(expected_duty)

        def sorting_key(x) -> str:
            return x["payload"]["deposit_request_outpoint"]

        duties.sort(key=sorting_key)
        expected_duties.sort(key=sorting_key)

        assert len(duties) == len(expected_duties), "num duties must match"
        assert duties == expected_duties, "duties in response should match expected ones"
        assert duties_resp["start_index"] == start_index, "start index must match"
        assert (
            duties_resp["stop_index"] > start_index
        ), "stop_index must be greater than start_index"
