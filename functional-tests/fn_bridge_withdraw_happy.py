import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import (
    deposit_request_transaction,
    extract_p2tr_pubkey,
    get_address,
    get_balance,
)
from web3 import Web3

from constants import (
    DEFAULT_BLOCK_TIME_SEC,
    DEFAULT_ROLLUP_PARAMS,
    GWEI_TO_WEI,
    PRECOMPILE_BRIDGEOUT_ADDRESS,
    SATS_TO_WEI,
    UNSPENDABLE_ADDRESS,
)
from entry import BasicEnvConfig
from utils import get_bridge_pubkey, get_logger

# Local constants
# D BTC
ROLLUP_DEPOSIT_AMOUNT = DEFAULT_ROLLUP_PARAMS["deposit_amount"]
# Gas for the withdrawal transaction
GAS = 22_000
# lower bound is D BTC - fees
STRATA_DEPOSIT_LOWER_BOUND = ROLLUP_DEPOSIT_AMOUNT * SATS_TO_WEI - GAS * GWEI_TO_WEI


@flexitest.register
class BridgeWithdrawHappyTest(flexitest.Test):
    """
    Makes two DRT deposits to the same EL address, then makes a withdrawal to a change address.

    Checks if the balance of the EL address is expected
    and if the BTC balance of the change address is expected.
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(pre_generate_blocks=101))
        self.logger = get_logger("BridgeWithdrawHappyTest")

    def main(self, ctx: flexitest.RunContext):
        address = get_address(0)
        withdraw_address = get_address(1)
        self.logger.debug(f"Address: {address}")
        self.logger.debug(f"Change Address: {withdraw_address}")
        self.logger.debug(f"Gas: {GAS}")

        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        reth = ctx.get_service("reth")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()
        rethrpc = reth.create_rpc()

        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]

        self.logger.debug(f"BTC URL: {btc_url}")
        self.logger.debug(f"BTC user: {btc_user}")
        self.logger.debug(f"BTC password: {btc_password}")

        web3: Web3 = reth.create_web3()
        web3.eth.default_account = web3.address
        el_address = web3.address
        self.logger.debug(f"EL address: {el_address}")

        # Get the balance of the EL address prior to the deposits
        # FIXME: I have no idea why the initial balance is so high
        balance_pre = int(rethrpc.eth_getBalance(el_address), 16)
        self.logger.debug(f"Strata Balance before deposits: {balance_pre}")
        assert (
            balance_pre == ROLLUP_DEPOSIT_AMOUNT * SATS_TO_WEI * 100_000
        ), "Strata balance is not expected"

        # Get operators pubkey and musig2 aggregates it
        bridge_pk = get_bridge_pubkey(seqrpc)
        self.logger.debug(f"Bridge pubkey: {bridge_pk}")

        seq_addr = seq.get_prop("address")
        self.logger.debug(f"Sequencer Address: {seq_addr}")
        bridge_pk = get_bridge_pubkey(seqrpc)

        # Generate Plenty of BTC to address
        btcrpc.proxy.generatetoaddress(102, address)

        # Deposit to the EL address
        # NOTE: we need 2 deposits to make sure we have funds for gas
        self.make_drt(ctx, el_address, bridge_pk)
        self.make_drt(ctx, el_address, bridge_pk)
        time.sleep(0.5)

        # Get the balance of the EL address prior to the withdrawal
        balance_post = int(rethrpc.eth_getBalance(el_address), 16)
        self.logger.debug(f"Strata Balance after deposits: {balance_post}")
        assert balance_post > 0, "Strata balance is not greater than 0"

        # Send funds to the bridge precompile address for a withdrawal
        change_address_pk = extract_p2tr_pubkey(withdraw_address)
        self.logger.debug(f"Change Address PK: {change_address_pk}")
        l2_tx_hash = self.make_withdraw(ctx, change_address_pk, GAS).hex()
        self.logger.debug(f"Sent withdrawal transaction with hash: {l2_tx_hash}")

        # Wait for the withdrawal to be processed
        time.sleep(DEFAULT_BLOCK_TIME_SEC * 2)

        l2_tx_state = web3.eth.get_transaction_receipt(l2_tx_hash)
        self.logger.debug(f"Transaction state (web3): {l2_tx_state}")

        # Make sure that the balance is less than the initial balance
        balance_post_withdraw = int(rethrpc.eth_getBalance(el_address), 16)
        self.logger.debug(f"Strata Balance after withdrawal: {balance_post_withdraw}")
        difference = balance_post - balance_post_withdraw
        self.logger.debug(f"Strata Balance difference: {difference}")
        # NOTE: Difference is D BTC - gas_fee
        assert difference == 10_000_023_000_818_888_144, "balance difference is not expected"

        # Mine blocks
        btcrpc.proxy.generatetoaddress(12, UNSPENDABLE_ADDRESS)
        time.sleep(1)

        # Make sure that the balance in the BTC wallet is D BTC-ish
        btc_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
        self.logger.debug(f"BTC balance: {btc_balance}")
        btc_balance_lower_bound = ROLLUP_DEPOSIT_AMOUNT // 1.075
        self.logger.debug(f"BTC expected balance lower bound: {btc_balance_lower_bound}")
        assert btc_balance >= btc_balance_lower_bound, "BTC balance is not D BTC"

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
        self.logger.debug(f"Deposit request tx: {tx}")

        # Send the transaction to the Bitcoin network
        txid = btcrpc.proxy.sendrawtransaction(tx)
        self.logger.debug(f"sent deposit request with txid = {txid} for address {el_address}")
        time.sleep(1)

        # time to mature DRT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

        # time to mature DT
        btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

    def make_withdraw(self, ctx: flexitest.RunContext, change_address_pk, gas=GAS):
        """
        Withdrawal Request Transaction in Strata's EVM.
        """
        reth = ctx.get_service("reth")
        web3: Web3 = reth.create_web3()
        web3.eth.default_account = web3.address
        el_address = web3.address
        self.logger.debug(f"EL address: {el_address}")
        self.logger.debug(f"Bridge address: {PRECOMPILE_BRIDGEOUT_ADDRESS}")

        data_bytes = bytes.fromhex(change_address_pk)

        transaction = {
            "from": el_address,
            "to": PRECOMPILE_BRIDGEOUT_ADDRESS,
            "value": ROLLUP_DEPOSIT_AMOUNT * SATS_TO_WEI,
            "gas": gas,
            "data": data_bytes,
        }
        l2_tx_hash = web3.eth.send_transaction(transaction)
        return l2_tx_hash
