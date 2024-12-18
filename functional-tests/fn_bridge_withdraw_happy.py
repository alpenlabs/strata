import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import (
    deposit_request_transaction,
    extract_p2tr_pubkey,
    get_balance,
)
from web3 import Web3
from web3.middleware import SignAndSendRawMiddlewareBuilder

import testenv
from constants import (
    DEFAULT_ROLLUP_PARAMS,
    PRECOMPILE_BRIDGEOUT_ADDRESS,
    SATS_TO_WEI,
    UNSPENDABLE_ADDRESS,
)
from utils import get_bridge_pubkey, wait_until

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
# BTC extra fee for withdrawal
WITHDRAWAL_EXTRA_FEE = DEFAULT_ROLLUP_PARAMS["withdraw_extra_fee"]


@flexitest.register
class BridgeWithdrawHappyTest(testenv.StrataTester):
    """
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
        self.debug(f"Gas: {WITHDRAWAL_GAS_FEE}")

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

        # Gas price
        gas_price = web3.to_wei(1, "gwei")
        self.debug(f"Gas price: {gas_price}")

        # Get operators pubkey and musig2 aggregates it
        bridge_pk = get_bridge_pubkey(seqrpc)
        self.debug(f"Bridge pubkey: {bridge_pk}")

        # Deposit to the EL address
        # NOTE: we need 2 deposits to make sure we have funds for gas
        self.make_drt(ctx, el_address, bridge_pk)
        self.make_drt(ctx, el_address, bridge_pk)
        balance_after_deposits = int(rethrpc.eth_getBalance(el_address), 16)
        self.debug(f"Strata Balance after deposits: {balance_after_deposits}")
        wait_until(
            lambda: int(rethrpc.eth_getBalance(el_address), 16) == 2 * DEPOSIT_AMOUNT * SATS_TO_WEI
        )

        # Get the balance of the EL address after the deposits
        balance = int(rethrpc.eth_getBalance(el_address), 16)
        self.debug(f"Strata Balance after deposits: {balance}")
        assert balance == 2 * DEPOSIT_AMOUNT * SATS_TO_WEI, "Strata balance is not expected"

        # Send funds to the bridge precompile address for a withdrawal
        change_address_pk = extract_p2tr_pubkey(withdraw_address)
        self.debug(f"Change Address PK: {change_address_pk}")
        estimated_withdraw_gas = self.estimate_withdraw_gas(
            ctx, web3, el_address, change_address_pk
        )
        self.debug(f"Estimated withdraw gas: {estimated_withdraw_gas}")
        l2_tx_hash = self.make_withdraw(
            ctx, web3, el_address, change_address_pk, estimated_withdraw_gas
        ).hex()
        self.debug(f"Sent withdrawal transaction with hash: {l2_tx_hash}")

        # Wait for the withdrawal to be processed
        wait_until(lambda: web3.eth.get_transaction_receipt(l2_tx_hash))
        tx_receipt = web3.eth.get_transaction_receipt(l2_tx_hash)
        self.debug(f"Transaction receipt: {tx_receipt}")
        total_gas_used = tx_receipt["gasUsed"] * tx_receipt["effectiveGasPrice"]
        self.debug(f"Total gas used: {total_gas_used}")

        # Make sure that the balance is expected
        balance_post_withdraw = int(rethrpc.eth_getBalance(el_address), 16)
        self.debug(f"Strata Balance after withdrawal: {balance_post_withdraw}")
        difference = DEPOSIT_AMOUNT * SATS_TO_WEI - total_gas_used
        self.debug(f"Strata Balance difference: {difference}")
        assert difference == balance_post_withdraw, "balance difference is not expected"

        # Wait for the withdraw address to have a positive balance
        self.mine_blocks_until_maturity(
            btcrpc, withdraw_address, btc_url, btc_user, btc_password, original_balance
        )

        # Make sure that the balance in the BTC wallet is D BTC - operator's fees
        btc_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
        self.debug(f"BTC balance: {btc_balance}")
        difference = DEPOSIT_AMOUNT - OPERATOR_FEE - WITHDRAWAL_EXTRA_FEE
        self.debug(f"BTC expected balance: {original_balance + difference}")
        assert btc_balance == original_balance + difference, "BTC balance is not expected"

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
        time.sleep(1)

        # time to mature DRT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

        # time to mature DT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

    def make_withdraw(
        self,
        ctx: flexitest.RunContext,
        web3: Web3,
        el_address,
        change_address_pk,
        gas=WITHDRAWAL_GAS_FEE,
    ):
        """
        Withdrawal Request Transaction in Strata's EVM.
        """
        self.debug(f"EL address: {el_address}")
        self.debug(f"Bridge address: {PRECOMPILE_BRIDGEOUT_ADDRESS}")

        data_bytes = bytes.fromhex(change_address_pk)

        transaction = {
            "from": el_address,
            "to": PRECOMPILE_BRIDGEOUT_ADDRESS,
            "value": DEPOSIT_AMOUNT * SATS_TO_WEI,
            "gas": gas,
            "data": data_bytes,
        }
        l2_tx_hash = web3.eth.send_transaction(transaction)
        return l2_tx_hash

    def estimate_withdraw_gas(
        self, ctx: flexitest.RunContext, web3: Web3, el_address, change_address_pk
    ):
        """
        Estimate the gas for the withdrawal transaction.
        """
        self.debug(f"EL address: {el_address}")
        self.debug(f"Bridge address: {PRECOMPILE_BRIDGEOUT_ADDRESS}")

        data_bytes = bytes.fromhex(change_address_pk)

        transaction = {
            "from": el_address,
            "to": PRECOMPILE_BRIDGEOUT_ADDRESS,
            "value": DEPOSIT_AMOUNT * SATS_TO_WEI,
            "data": data_bytes,
        }
        return web3.eth.estimate_gas(transaction)

    def mine_blocks_until_maturity(
        self,
        btcrpc,
        withdraw_address,
        btc_url,
        btc_user,
        btc_password,
        original_balance,
        number_of_blocks=12,
    ):
        """
        Mine blocks until the withdraw address has a positive balance
        By default, the number of blocks to mine is 12:
        - 6 blocks to mature the DRT
        - 6 blocks to mature the DT
        """
        btcrpc.proxy.generatetoaddress(number_of_blocks, UNSPENDABLE_ADDRESS)
        wait_until(
            lambda: get_balance(withdraw_address, btc_url, btc_user, btc_password)
            > original_balance
        )
