import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import (
    deposit_request_transaction,
    get_balance,
)
from web3 import Web3
from web3.middleware import SignAndSendRawMiddlewareBuilder

import testenv
from constants import (
    DEFAULT_ROLLUP_PARAMS,
    SATS_TO_WEI,
)
from utils import get_bridge_pubkey, wait_until, wait_until_with_value

# Local constants
# D BTC
DEPOSIT_AMOUNT = DEFAULT_ROLLUP_PARAMS["deposit_amount"]
# Gas for the withdrawal transaction
WITHDRAWAL_GAS_FEE = 22_000  # technically is 21_000
# Ethereum Private Key
# NOTE: don't use this private key in production
ETH_PRIVATE_KEY = "0x0000000000000000000000000000000000000000000000000000000000000001"
# BTC Operator's fee for withdrawal
OPERATOR_FEE = DEFAULT_ROLLUP_PARAMS["operator_fee"]


@flexitest.register
class BridgeDepositSequencerUnreliableTest(testenv.StrataTester):
    """
    TODO: Depends on STR-734 operator reassignment, and this can be merged only that is merged

    Makes two DRT deposits to the same EL address, then makes a withdrawal to a change address.

    Checks if the balance of the EL address is expected
    and if the BTC balance of the change address is expected.
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        address = ctx.env.gen_ext_btc_address()
        withdraw_address = ctx.env.gen_ext_btc_address()
        self.debug(f"Address: {address}")
        self.debug(f"Change Address: {withdraw_address}")

        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        reth = ctx.get_service("reth")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()
        rethrpc = reth.create_rpc()

        seq_addr = seq.get_prop("address")
        self.debug(f"Sequencer Address: {seq_addr}")

        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]

        self.debug(f"BTC URL: {btc_url}")
        self.debug(f"BTC user: {btc_user}")
        self.debug(f"BTC password: {btc_password}")

        # Get the original balance of the withdraw address
        original_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
        self.debug(f"BTC balance before withdraw: {original_balance}")

        web3: Web3 = reth.create_web3()
        # Create an Ethereum account from the private key
        eth_account = web3.eth.account.from_key(ETH_PRIVATE_KEY)
        el_address = eth_account.address
        self.debug(f"EL address: {el_address}")

        # Add the Ethereum account as auto-signer
        # Transactions from `el_address` will then be signed, under the hood, in the middleware
        web3.middleware_onion.inject(SignAndSendRawMiddlewareBuilder.build(eth_account), layer=0)

        # Get the balance of the EL address before the deposits
        balance = int(rethrpc.eth_getBalance(el_address), 16)
        self.debug(f"Strata Balance before deposits: {balance}")
        assert balance == 0, "Strata balance is not expected"

        # Get operators pubkey and musig2 aggregates it
        bridge_pk = get_bridge_pubkey(seqrpc)
        self.debug(f"Bridge pubkey: {bridge_pk}")
        self.debug("Stopping the sequencer")

        self.make_drt(ctx, el_address, bridge_pk)
        time.sleep(2)

        # stop sequencer
        seq.stop()
        time.sleep(1)

        self.make_drt(ctx, el_address, bridge_pk)

        # start again
        seq.start()

        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
            timeout=10,
        )

        balance_after_deposits = wait_until_with_value(
            lambda: int(rethrpc.eth_getBalance(el_address), 16),
            predicate=lambda v: v == 2 * DEPOSIT_AMOUNT * SATS_TO_WEI,
            timeout=15,
        )
        self.debug(f"Strata Balance after deposits: {balance_after_deposits}")

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
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]
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
        # time to mature DRT
        btcrpc.proxy.generatetoaddress(6, seq_addr)

        # time to mature DT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
