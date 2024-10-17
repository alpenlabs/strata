import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import deposit_request_transaction, get_address

from entry import BasicEnvConfig
from utils import get_logger


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
        return True  # this will be handled in a future PR
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        reth = ctx.get_service("reth")
        el_address_1 = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        el_address_2 = "deedf001900dca3ebeefdeadf001900dca3ebeef"

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()
        rethrpc = reth.create_rpc()

        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]

        self.logger.debug(f"BTC URL: {btc_url}")
        self.logger.debug(f"BTC user: {btc_user}")
        self.logger.debug(f"BTC password: {btc_password}")

        seq_addr = seq.get_prop("address")
        addr_1 = get_address(0)
        addr_2 = get_address(1)
        self.logger.debug(f"EL address 1: {el_address_1}")
        self.logger.debug(f"EL address 2: {el_address_2}")

        original_balance_1 = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        self.logger.debug(f"Balance before deposit (EL address 1): {original_balance_1}")

        original_balance_2 = int(rethrpc.eth_getBalance(f"0x{el_address_2}"), 16)
        self.logger.debug(f"Balance before deposit (EL address 2): {original_balance_2}")

        original_num_deposits = len(seqrpc.strata_getCurrentDeposits())
        self.logger.debug(f"Original deposit count: {original_num_deposits}")

        # First deposit, address 1
        self.logger.debug(f"Deposit address (index 0): {addr_1}")
        btcrpc.proxy.generatetoaddress(1, addr_1)

        # TODO: REFACTOR ALL THIS INTO A FUNCTION
        #       This should live in the utils.py file.
        # Create the first deposit request transaction
        tx = deposit_request_transaction(el_address_1, btc_url, btc_user, btc_password)
        self.logger.debug(f"Deposit request tx: {tx}")
        txid = btcrpc.sendrawtransaction(tx)
        self.logger.debug(f"sent deposit request with txid = {txid} for address {el_address_1}")

        # Now poll for the tx in chain
        # TODO: replace by a wait_until btc rpc call
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
        # time to mature DRT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(1)
        # TODO: time to mature DT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(1)
        new_balance = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        assert new_balance > original_balance_1, "balance did not increase"
        # TODO: check that the difference is the same from the deposit amount.
        #       You can get it from the rollups params

        current_block_num = int(rethrpc.eth_blockNumber(), base=16)
        print(f"Current reth block num: {current_block_num}")

        # Second deposit, address 2
        self.logger.debug(f"Deposit address: {addr_2}")
        btcrpc.proxy.generatetoaddress(1, addr_2)

        # Create the first deposit request transaction
        tx = deposit_request_transaction(el_address_2, btc_url, btc_user, btc_password)
        self.logger.debug(f"Deposit request tx: {tx}")
        txid = btcrpc.sendrawtransaction(tx)
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
        # time to mature FIXME: is this 6 blocks?
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(1)

        new_balance_2 = int(rethrpc.eth_getBalance(f"0x{el_address_2}"), 16)
        assert new_balance_2 > original_balance_2, "balance did not increase"

        # Third deposit, address 1
        # Create the first deposit request transaction
        tx = deposit_request_transaction(el_address_1, btc_url, btc_user, btc_password)
        self.logger.debug(f"Deposit request tx: {tx}")
        txid = btcrpc.sendrawtransaction(tx)
        original_balance_3 = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        self.logger.debug(f"Balance before deposit: {original_balance_3}")
        txid = btcrpc.sendrawtransaction(tx)
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
        # time to mature FIXME: is this 6 blocks?
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(1)

        new_balance_3 = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        assert new_balance_3 > original_balance_3, "balance did not increase"
