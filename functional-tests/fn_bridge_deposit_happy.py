import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from constants import DEFAULT_ROLLUP_PARAMS
from entry import BasicEnvConfig
from utils import broadcast_tx, get_logger


@flexitest.register
class BridgeDepositHappyTest(flexitest.Test):
    """
    A test class for happy path scenarios of bridge deposits.

    It tests the functionality of depositing Bitcoin into the bridge and verifying the
    corresponding increase in balance on the Ethereum side.

    Deposit funds into the rollup, happy path,
    check that funds can be swept with a normal Ethereum transaction afterwards.
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(101))
        self.logger = get_logger("BridgeDepositHappyTest")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        el_address_1 = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        el_address_2 = "deedf001900dca3ebeefdeadf001900dca3ebeef"

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        original_balance = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        self.logger.debug(f"Balance before deposit: {original_balance}")

        original_balance_2 = int(rethrpc.eth_getBalance(f"0x{el_address_2}"), 16)
        self.logger.debug(f"Balance before deposit: {original_balance_2}")

        original_num_deposits = len(seqrpc.strata_getCurrentDeposits())
        self.logger.debug(f"Original deposit count: {original_num_deposits}")

        fees_in_btc = 0.01
        sats_per_btc = 10**8
        amount_to_send = DEFAULT_ROLLUP_PARAMS["deposit_amount"] / sats_per_btc + fees_in_btc
        take_back_leaf_hash = "02" * 32
        magic_bytes = DEFAULT_ROLLUP_PARAMS["rollup_name"].encode("utf-8").hex()
        options = {"changePosition": 2}

        current_block_num = int(rethrpc.eth_blockNumber(), base=16)
        print(f"Current reth block num: {current_block_num}")

        # First deposit address 1
        addr_1 = btcrpc.proxy.getnewaddress("", "bech32m")
        btcrpc.proxy.generatetoaddress(1, addr_1)
        outputs = [
            {addr_1: amount_to_send},
            {"data": f"{magic_bytes}{take_back_leaf_hash}{el_address_1}"},
        ]
        txid = broadcast_tx(btcrpc, outputs, options)
        self.logger.debug(f"sent deposit request with txid = {txid} for address {el_address_1}")
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
        # time to mature FIXME: is this 3 blocks?
        btcrpc.proxy.generatetoaddress(3, addr_1)
        time.sleep(1)
        new_balance = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        assert new_balance > original_balance, "balance did not increase"

        current_block_num = int(rethrpc.eth_blockNumber(), base=16)
        print(f"Current reth block num: {current_block_num}")

        # Second deposit address 2
        addr_2 = btcrpc.proxy.getnewaddress("", "bech32m")
        btcrpc.proxy.generatetoaddress(1, addr_2)
        outputs = [
            {addr_2: amount_to_send},
            {"data": f"{magic_bytes}{take_back_leaf_hash}{el_address_2}"},
        ]
        txid = broadcast_tx(btcrpc, outputs, options)
        self.logger.debug(f"sent deposit request with txid = {txid} for address {el_address_2}")
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
        # time to mature FIXME: is this 3 blocks?
        btcrpc.proxy.generatetoaddress(3, addr_2)
        time.sleep(1)
        new_balance_2 = int(rethrpc.eth_getBalance(f"0x{el_address_2}"), 16)
        assert new_balance_2 > original_balance_2, "balance did not increase"

        # Third deposit address 1
        original_balance_3 = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        self.logger.debug(f"Balance before deposit: {original_balance}")
        addr_1 = btcrpc.proxy.getnewaddress("", "bech32m")
        btcrpc.proxy.generatetoaddress(1, addr_1)
        take_back_leaf_hash = "02" * 32
        magic_bytes = DEFAULT_ROLLUP_PARAMS["rollup_name"].encode("utf-8").hex()
        outputs = [
            {addr_1: amount_to_send},
            {"data": f"{magic_bytes}{take_back_leaf_hash}{el_address_1}"},
        ]
        txid = broadcast_tx(btcrpc, outputs, options)
        self.logger.debug(f"sent deposit request with txid = {txid} for address {el_address_1}")
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
        new_balance = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        assert new_balance > original_balance_3, "balance did not increase"
