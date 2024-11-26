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
    DEFAULT_ROLLUP_PARAMS,
    PRECOMPILE_BRIDGEOUT_ADDRESS,
    SATS_TO_WEI,
    UNSPENDABLE_ADDRESS,
)
from entry import BasicEnvConfig
from utils import get_bridge_pubkey, get_logger, wait_until

# Local constants
# D BTC
DEPOSIT_AMOUNT = DEFAULT_ROLLUP_PARAMS["deposit_amount"]
# Gas for the withdrawal transaction
WITHDRAWAL_GAS_FEE = 22_000  # technically is 21_000
# Burner address in Strata
BURNER_ADDRESS = Web3.to_checksum_address("0xdeadf001900dca3ebeefdeadf001900dca3ebeef")
# BTC Operator's fee for withdrawal
OPERATOR_FEE = DEFAULT_ROLLUP_PARAMS["operator_fee"]
# BTC extra fee for withdrawal
WITHDRAWAL_EXTRA_FEE = DEFAULT_ROLLUP_PARAMS["withdraw_extra_fee"]


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

    # TODO: This needs refactoring to be more clear
    def main(self, ctx: flexitest.RunContext):
        address = get_address(0)
        withdraw_address = get_address(1)
        self.logger.debug(f"Address: {address}")
        self.logger.debug(f"Change Address: {withdraw_address}")
        self.logger.debug(f"Gas: {WITHDRAWAL_GAS_FEE}")

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

        # Gas price
        gas_price = web3.to_wei(1, "gwei")
        self.logger.debug(f"Gas price: {gas_price}")

        # FIXME: Somehow web3 default account has some balance.
        # Hence we need to set it to burn it.
        balance_to_burn = int(rethrpc.eth_getBalance(el_address), 16)
        self.logger.debug(f"Strata balance to burn: {balance_to_burn}")
        estimated_gas = web3.eth.estimate_gas(
            {
                "from": el_address,
                "to": BURNER_ADDRESS,
                "value": balance_to_burn,
            }
        )
        web3.eth.send_transaction(
            {
                "from": el_address,
                "to": BURNER_ADDRESS,
                "value": balance_to_burn - (estimated_gas * gas_price),
                "gas": estimated_gas,
                "gasPrice": gas_price,
            }
        )
        wait_until(lambda: int(rethrpc.eth_getBalance(el_address), 16) == 0)
        # Ok now we have a clean state
        self.logger.debug("Strata balance is zero")

        # Get operators pubkey and musig2 aggregates it
        bridge_pk = get_bridge_pubkey(seqrpc)
        self.logger.debug(f"Bridge pubkey: {bridge_pk}")

        seq_addr = seq.get_prop("address")
        self.logger.debug(f"Sequencer Address: {seq_addr}")
        bridge_pk = get_bridge_pubkey(seqrpc)

        # Generate plenty of BTC to address
        btcrpc.proxy.generatetoaddress(102, address)

        # Deposit to the EL address
        # NOTE: we need 2 deposits to make sure we have funds for gas
        self.make_drt(ctx, el_address, bridge_pk)
        self.make_drt(ctx, el_address, bridge_pk)
        wait_until(
            lambda: int(rethrpc.eth_getBalance(el_address), 16) == 2 * DEPOSIT_AMOUNT * SATS_TO_WEI
        )

        # Get the balance of the EL address after the deposits
        balance = int(rethrpc.eth_getBalance(el_address), 16)
        self.logger.debug(f"Strata Balance after deposits: {balance}")
        assert balance == 2 * DEPOSIT_AMOUNT * SATS_TO_WEI, "Strata balance is not expected"

        # Send funds to the bridge precompile address for a withdrawal
        change_address_pk = extract_p2tr_pubkey(withdraw_address)
        self.logger.debug(f"Change Address PK: {change_address_pk}")
        estimated_withdraw_gas = self.estimate_withdraw_gas(ctx, change_address_pk)
        self.logger.debug(f"Estimated withdraw gas: {estimated_withdraw_gas}")
        l2_tx_hash = self.make_withdraw(ctx, change_address_pk, estimated_withdraw_gas).hex()
        self.logger.debug(f"Sent withdrawal transaction with hash: {l2_tx_hash}")

        # Wait for the withdrawal to be processed
        wait_until(lambda: web3.eth.get_transaction_receipt(l2_tx_hash))
        tx_receipt = web3.eth.get_transaction_receipt(l2_tx_hash)
        self.logger.debug(f"Transaction receipt: {tx_receipt}")
        total_gas_used = tx_receipt["gasUsed"] * tx_receipt["effectiveGasPrice"]
        self.logger.debug(f"Total gas used: {total_gas_used}")

        # Make sure that the balance is expected
        balance_post_withdraw = int(rethrpc.eth_getBalance(el_address), 16)
        self.logger.debug(f"Strata Balance after withdrawal: {balance_post_withdraw}")
        difference = DEPOSIT_AMOUNT * SATS_TO_WEI - total_gas_used
        self.logger.debug(f"Strata Balance difference: {difference}")
        assert difference == balance_post_withdraw, "balance difference is not expected"

        # Mine blocks
        btcrpc.proxy.generatetoaddress(12, UNSPENDABLE_ADDRESS)
        wait_until(lambda: get_balance(withdraw_address, btc_url, btc_user, btc_password) > 0)

        # Make sure that the balance in the BTC wallet is D BTC - operator's fees
        btc_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
        self.logger.debug(f"BTC balance: {btc_balance}")
        expected_balance = DEPOSIT_AMOUNT - OPERATOR_FEE - WITHDRAWAL_EXTRA_FEE
        self.logger.debug(f"BTC expected balance: {expected_balance}")
        assert btc_balance == expected_balance, "BTC balance is not expected"

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

    def make_withdraw(self, ctx: flexitest.RunContext, change_address_pk, gas=WITHDRAWAL_GAS_FEE):
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
            "value": DEPOSIT_AMOUNT * SATS_TO_WEI,
            "gas": gas,
            "data": data_bytes,
        }
        l2_tx_hash = web3.eth.send_transaction(transaction)
        return l2_tx_hash

    def estimate_withdraw_gas(self, ctx: flexitest.RunContext, change_address_pk):
        """
        Estimate the gas for the withdrawal transaction.
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
            "value": DEPOSIT_AMOUNT * SATS_TO_WEI,
            "data": data_bytes,
        }
        return web3.eth.estimate_gas(transaction)
