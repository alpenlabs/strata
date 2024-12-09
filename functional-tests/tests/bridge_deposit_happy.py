import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import deposit_request_transaction, drain_wallet

from envs import testenv
from envs.testenv import BasicEnvConfig
from utils import *


@flexitest.register
class BridgeDepositHappyTest(testenv.StrataTester):
    """
    A test class for happy path scenarios of bridge deposits.

    It tests the functionality of depositing Bitcoin into the bridge and verifying the
    corresponding increase in balance on the Ethereum side.

    Deposit funds into the rollup, happy path,
    check that funds can be swept with a normal Ethereum transaction afterwards.
    """

    def __init__(self, ctx: flexitest.InitContext):
        # Note: using copy of basic env her to have independent sequencer for this test
        ctx.set_env(BasicEnvConfig(101))

    def main(self, ctx: flexitest.RunContext):
        el_address_1 = ctx.env.gen_el_address()
        el_address_2 = ctx.env.gen_el_address()

        addr_1 = ctx.env.gen_ext_btc_address()
        addr_2 = ctx.env.gen_ext_btc_address()
        addr_3 = ctx.env.gen_ext_btc_address()

        # 1st deposit
        self.test_deposit(ctx, addr_1, el_address_1)
        time.sleep(0.5)

        # 2nd deposit
        self.test_deposit(ctx, addr_2, el_address_2)
        time.sleep(0.5)

        # 3rd deposit, now to a previously used address
        self.test_deposit(ctx, addr_3, el_address_1, new_address=False)
        time.sleep(0.5)

        return True

    def make_drt(self, ctx: flexitest.RunContext, el_address, musig_bridge_pk):
        """
        Deposit Request Transaction
        """
        # Get relevant data
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.get_prop("rpc_user")
        btc_password = btc.get_prop("rpc_password")
        seq_addr = seq.get_prop("address")

        # Create the deposit request transaction
        tx = bytes(
            deposit_request_transaction(
                el_address, musig_bridge_pk, btc_url, btc_user, btc_password
            )
        ).hex()
        self.debug(f"Deposit request tx: {tx}")

        # Send the transaction to the Bitcoin network
        txid = btcrpc.proxy.sendrawtransaction(tx)
        self.debug(f"sent deposit request with txid = {txid} for address {el_address}")
        # this transaction is not in the bitcoind wallet, so we cannot use gettransaction
        time.sleep(1)

        # time to mature DRT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

        # time to mature DT
        btcrpc.proxy.generatetoaddress(6, seq_addr)

        # time for the deposit inclusion
        timeout = 70 # TODO needs to be until the end of the epoch
        wait_until_checkpoint(seq.create_rpc(), timeout)

    def drain_wallet(self, ctx: flexitest.RunContext):
        """
        Drains the wallet to the sequencer address
        """
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.get_prop("rpc_user")
        btc_password = btc.get_prop("rpc_password")
        seq_addr = seq.get_prop("address")

        tx = bytes(drain_wallet(seq_addr, btc_url, btc_user, btc_password)).hex()

        txid = btcrpc.proxy.sendrawtransaction(tx)
        # this transaction is not in the bitcoind wallet, so we cannot use gettransaction
        time.sleep(1)
        self.debug(f"drained wallet back to sequencer, txid: {txid}")

        return txid

    def test_deposit(
        self, ctx: flexitest.RunContext, address: str, el_address: str, new_address=True
    ):
        """
        Test depositing funds into the bridge and verifying the corresponding increase in balance
        on the Strata side.
        """
        rollup_deposit_amount = ctx.env.rollup_cfg().deposit_amount

        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        reth = ctx.get_service("reth")

        self.debug(f"EL address: {el_address}")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()
        rethrpc = reth.create_rpc()

        btc_url = btcrpc.base_url
        btc_user = btc.get_prop("rpc_user")
        btc_password = btc.get_prop("rpc_password")

        self.debug(f"BTC URL: {btc_url}")
        self.debug(f"BTC user: {btc_user}")
        self.debug(f"BTC password: {btc_password}")

        # Get operators pubkey and musig2 aggregates it
        bridge_pk = get_bridge_pubkey(seqrpc)
        self.debug(f"Bridge pubkey: {bridge_pk}")

        seq_addr = seq.get_prop("address")
        self.debug(f"Sequencer Address: {seq_addr}")
        self.debug(f"Address: {address}")

        n_deposits_pre = len(seqrpc.strata_getCurrentDeposits())
        self.debug(f"Current deposits: {n_deposits_pre}")

        # Make sure that the el_address has zero balance
        original_balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        self.debug(f"Balance before deposit (EL address): {original_balance}")

        if new_address:
            assert original_balance == 0, "balance is not zero"
        else:
            assert original_balance > 0, "balance is zero"

        # Send DRT from Address 1 to EL Address 1
        self.make_drt(ctx, el_address, bridge_pk)
        # Make sure that the n_deposits is correct

        n_deposits_post = len(seqrpc.strata_getCurrentDeposits())
        self.debug(f"Current deposits: {n_deposits_post}")
        assert n_deposits_post == n_deposits_pre + 1, "deposit was not registered"

        # Make sure that the balance has increased
        time.sleep(0.5)
        new_balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        self.debug(f"Balance after deposit (EL address): {new_balance}")
        assert new_balance > original_balance, "balance did not increase"

        # Make sure that the balance is the default deposit amount of BTC in Strata "wei"
        assert new_balance - original_balance == rollup_deposit_amount * (
            10**10
        ), "balance is not the default rollup_deposit_amount"
