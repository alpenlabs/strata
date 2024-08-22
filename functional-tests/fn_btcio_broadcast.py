import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient


@flexitest.register
class BroadcastTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("premined_blocks")  # premined because we want to create a tx

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
        print("Signed Tx", signed_tx)

        txid = seqrpc.alpadmin_broadcastRawTx(signed_tx)
        print("Rpc returned txid", txid)

        # Now poll for the tx in chain
        tx_published = False
        for _ in range(10):
            time.sleep(1)
            try:
                _ = btcrpc.gettransaction(txid)
                print("Found expected tx in mempool")
                tx_published = True
                break
            except Exception as e:
                print(e)
        assert tx_published, "Tx was not published"

        # Also check from rpc, wait for a while
        time.sleep(1)
        st = seqrpc.alp_getTxStatus(txid)
        assert st["status"] == "Confirmed"

        return True
