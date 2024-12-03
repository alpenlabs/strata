import logging
from pathlib import Path

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from utils import wait_until_with_value


@flexitest.register
class BroadcastTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")
        self.logger = logging.getLogger(Path(__file__).stem)

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")

        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        addr = seq.get_prop("address")

        unspent = btcrpc.getutxos(addr)

        # create inputs
        inputs = [{"txid": unspent[0]["txid"], "vout": 0}]
        send_amt = unspent[0]["value"] / 10**8 - 0.005  # 0.005 is the fee
        dest = [{addr: send_amt}]

        raw_tx = btcrpc.proxy.createrawtransaction(inputs, dest)

        signed_tx = btcrpc.proxy.signrawtransactionwithwallet(raw_tx)["hex"]
        self.logger.debug(f"Signed Tx {signed_tx}")

        txid = seqrpc.strataadmin_broadcastRawTx(signed_tx)
        self.logger.debug(f"Rpc returned txid {txid}")

        wait_until_with_value(
            lambda: btcrpc.gettransaction(txid),
            predicate=lambda v: v is not None,
            error_with="Tx was not published",
            timeout=10,
        )

        # Also check from strata rpc
        wait_until_with_value(
            lambda: seqrpc.strata_getTxStatus(txid),
            predicate=lambda v: v["status"] in ("Confirmed", "Finalized"),
            error_with="Tx was not identified by strata",
            timeout=3,
        )
        return True
