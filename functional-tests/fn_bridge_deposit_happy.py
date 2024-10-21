import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import deposit_request_transaction, drain_wallet, get_address

from constants import DEFAULT_ROLLUP_PARAMS
from entry import BasicEnvConfig
from utils import get_bridge_pubkey, get_logger


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
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        el_address_1 = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        el_address_2 = "deedf001900dca3ebeefdeadf001900dca3ebeef"

        addr_1 = get_address(0)
        addr_2 = get_address(1)

        # 1st deposit
        n_deposits_pre_1 = len(seqrpc.strata_getCurrentDeposits())
        self.logger.debug(f"Current deposits: {n_deposits_pre_1}")
        assert self.test_deposit(ctx, addr_1, el_address_1)
        n_deposits_post_1 = len(seqrpc.strata_getCurrentDeposits())
        self.logger.debug(f"Current deposits: {n_deposits_post_1}")
        assert n_deposits_post_1 == n_deposits_pre_1 + 1

        # 2nd deposit
        n_deposits_pre_2 = len(seqrpc.strata_getCurrentDeposits())
        self.logger.debug(f"Current deposits: {n_deposits_pre_2}")
        assert self.test_deposit(ctx, addr_2, el_address_2)
        n_deposits_post_2 = len(seqrpc.strata_getCurrentDeposits())
        self.logger.debug(f"Current deposits: {n_deposits_post_2}")
        assert n_deposits_post_2 == n_deposits_pre_2 + 1

    def make_drt(self, ctx: flexitest.RunContext, el_address, musig_bridge_pk):
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
        tx = bytes(
            deposit_request_transaction(
                el_address, musig_bridge_pk, btc_url, btc_user, btc_password
            )
        ).hex()
        self.logger.debug(f"Deposit request tx: {tx}")

        # Send the transaction to the Bitcoin network
        txid = btcrpc.proxy.sendrawtransaction(tx)
        self.logger.debug(f"sent deposit request with txid = {txid} for address {el_address}")
        # this transaction is not in the bitcoind wallet, so we cannot use gettransaction
        time.sleep(1)

        # time to mature DRT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

        # time to mature DT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

    def drain_wallet(self, ctx: flexitest.RunContext):
        """
        Drains the wallet to the sequencer address
        """
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]
        seq_addr = seq.get_prop("address")

        tx = bytes(drain_wallet(seq_addr, btc_url, btc_user, btc_password)).hex()

        txid = btcrpc.proxy.sendrawtransaction(tx)
        # this transaction is not in the bitcoind wallet, so we cannot use gettransaction
        time.sleep(1)
        self.logger.debug(f"drained wallet back to sequencer, txid: {txid}")

        return txid

    def test_deposit(self, ctx: flexitest.RunContext, address: str, el_address: str):
        """
        Test depositing funds into the bridge and verifying the corresponding increase in balance
        on the Strata side.
        """
        rollup_deposit_amount = DEFAULT_ROLLUP_PARAMS["deposit_amount"]

        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        reth = ctx.get_service("reth")

        self.logger.debug(f"EL address: {el_address}")

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
        self.logger.debug(f"Bridge pubkey: {bridge_pk}")

        seq_addr = seq.get_prop("address")
        self.logger.debug(f"Sequencer Address: {seq_addr}")
        self.logger.debug(f"Address: {address}")

        # Make sure that the el_address has zero balance
        original_balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        self.logger.debug(f"Balance before deposit (EL address): {original_balance}")
        assert original_balance == 0, "balance is not zero"

        # Generate Plenty of BTC to address
        btcrpc.proxy.generatetoaddress(102, address)

        # Send DRT from Address 1 to EL Address 1
        self.make_drt(ctx, el_address, bridge_pk)

        # Make sure that the balance has increased
        new_balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        self.logger.debug(f"Balance after deposit (EL address): {new_balance}")
        assert new_balance > original_balance, "balance did not increase"

        # Make sure that the balance is 10 BTC in Strata "wei"
        expected_balance = rollup_deposit_amount * (10**10)
        assert new_balance == expected_balance, "balance is not the default rollup_deposit_amount"

        # Drain wallet back to sequencer so that we cannot use address 1 or change anymore
        self.drain_wallet(ctx)
        return True
