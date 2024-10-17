import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import deposit_request_transaction, get_address, drain_wallet

from entry import BasicEnvConfig
from utils import get_logger, wait_until, get_bridge_pubkey


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
        reth = ctx.get_service("reth")

        el_address_1 = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        el_address_2 = "deedf001900dca3ebeefdeadf001900dca3ebeef"
        self.logger.debug(f"EL address 1: {el_address_1}")
        self.logger.debug(f"EL address 2: {el_address_2}")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()
        rethrpc = reth.create_rpc()

        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]

        self.logger.debug(f"BTC URL: {btc_url}")
        self.logger.debug(f"BTC user: {btc_user}")
        self.logger.debug(f"BTC password: {btc_password}")

        # Get operators pubkey and musig2 aggregates it
        bridge_pk = get_bridge_pubkey(seqrpc)
        print(f"Bridge pubkey: {bridge_pk}")

        seq_addr = seq.get_prop("address")
        addr_1 = get_address(0)
        addr_2 = get_address(1)
        self.logger.debug(f"Sequencer Address: {seq_addr}")
        self.logger.debug(f"Address 1: {addr_1}")
        self.logger.debug(f"Address 2: {addr_2}")

        # Generate 50 BTC to address 1
        tx = btcrpc.proxy.generatetoaddress(1, addr_1)
        self.logger.debug(f"Generated 50 BTC to address 1: {tx}")
        original_balance = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        self.logger.debug(f"Balance before deposit (EL address 1): {original_balance}")
        # Send DRT from Address 1 to EL Address 1
        self.drt(ctx, el_address_1, bridge_pk)
        new_balance = int(rethrpc.eth_getBalance(f"0x{el_address_1}"), 16)
        assert new_balance > original_balance, "balance did not increase"
        # Drain wallet back to sequencer so that we cannot use address 1 or change anymore
        tx = drain_wallet(seq_addr, btc_url, btc_user, btc_password)
        # TODO: Convert tx to hex and send it to the Bitcoin
        tx = tx.hex()
        btc.sendrawtransaction(tx)
        self.logger.debug(f"drained wallet tx: {tx}")
        # TODO: check that the difference is the same from the deposit amount.
        #       You can get it from the rollups params

        # Generate 50 BTC to address 2
        tx = btcrpc.proxy.generatetoaddress(1, addr_2)
        self.logger.debug(f"Generated 50 BTC to address 1: {tx}")
        original_balance = int(rethrpc.eth_getBalance(f"0x{el_address_2}"), 16)
        self.logger.debug(f"Balance before deposit (EL address 2): {original_balance}")
        # Send DRT from Address 2 to EL Address 2
        self.drt(ctx, el_address_2, bridge_pk)
        new_balance = int(rethrpc.eth_getBalance(f"0x{el_address_2}"), 16)
        assert new_balance > original_balance, "balance did not increase"
        # Drain wallet back to sequencer so that we cannot use address 2 or change anymore
        tx = drain_wallet(seq_addr, btc_url, btc_user, btc_password)
        # TODO: Convert tx to hex and send it to the Bitcoin
        tx = tx.hex()
        btc.sendrawtransaction(tx)
        self.logger.debug(f"drained wallet tx: {tx}")
        # TODO: check that the difference is the same from the deposit amount.
        #       You can get it from the rollups params

    def drt(self, ctx: flexitest.RunContext, el_address, musig_bridge_pk):
        """
        Deposit Request Transaction
        """
        # Get relevant data
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]
        seq_addr = seq.get_prop("address")

        # Create the deposit request transaction
        tx = deposit_request_transaction(
            el_address, musig_bridge_pk, btc_url, btc_user, btc_password
        )
        self.logger.debug(f"Deposit request tx: {tx}")

        # Send the transaction to the Bitcoin network
        txid = btcrpc.sendrawtransaction(tx)
        self.logger.debug(f"sent deposit request with txid = {txid} for address {el_address}")

        # Now poll for the tx in chain
        wait_until(
            lambda: btcrpc.gettransaction(txid),
            error_with="Tx was not published",
        )

        # time to mature DRT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(1)

        # time to mature DT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(1)
