import logging
from pathlib import Path

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import DEFAULT_ROLLUP_PARAMS, SEQ_PUBLISH_BATCH_INTERVAL_SECS
from utils import broadcast_tx, wait_until


@flexitest.register
class BridgeDutiesTest(flexitest.Test):
    """
    Test that the bridge client can fetch bridge duties correctly.
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")
        self.logger = logging.getLogger(Path(__file__).stem)

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()

        addr = ctx.env.gen_ext_btc_address()
        fees_in_btc = 0.01
        sats_per_btc = 10**8
        amount_to_send = DEFAULT_ROLLUP_PARAMS["deposit_amount"] / sats_per_btc + fees_in_btc

        el_address = ctx.env.gen_el_address()
        el_address_bytes = list(bytes.fromhex(el_address))
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

        # wait for the transactions to have at least 2 confirmations
        wait_until(
            lambda: all(btcrpc.proxy.gettransaction(txid)["confirmations"] >= 2 for txid in txids),
            timeout=SEQ_PUBLISH_BATCH_INTERVAL_SECS * 2,
        )

        operator_idx = 0
        start_index = 0
        self.logger.debug(
            f"getting bridge duties for operator_idx: {operator_idx} from index: {start_index}"
        )
        duties_resp = seqrpc.strata_getBridgeDuties(operator_idx, start_index)
        duties: list = duties_resp["duties"]
        # Filter out the duties unrelated to other than the el_address.
        for duty in duties:
            self.logger.debug(f"duty: {duty}")
            if "el_address" in duty["payload"]:
                self.logger.debug(f"duty['payload']['el_address']: {duty['payload']['el_address']}")
        duties = [
            d
            for d in duties
            if "el_address" in d["payload"] and d["payload"]["el_address"] == el_address_bytes
        ]

        expected_duties = []
        for txid in txids:
            expected_duty = {
                "type": "SignDeposit",
                "payload": {
                    "deposit_request_outpoint": f"{txid}:0",
                    "el_address": el_address_bytes,
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
