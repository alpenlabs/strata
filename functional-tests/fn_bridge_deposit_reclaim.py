import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import (
    deposit_request_transaction,
    get_address,
    get_balance,
    get_balance_recovery,
    get_recovery_address,
    take_back_transaction,
)

from constants import DEFAULT_ROLLUP_PARAMS, DEFAULT_TAKEBACK_TIMEOUT, UNSPENDABLE_ADDRESS
from entry import BasicEnvConfig
from utils import get_bridge_pubkey, get_logger


@flexitest.register
class BridgeDepositReclaimTest(flexitest.Test):
    """
    A test class for reclaim path scenarios of bridge deposits.

    It tests the functionality of broadcasting a deposit request transaction (DRT) for the bridge
    and verifying that the reclaim path works as expected.

    In the test we stop one of the bridge operators to prevent the DRT from being processed.
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(BasicEnvConfig(pre_generate_blocks=101))
        self.logger = get_logger("BridgeDepositReclaimTest")

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        reth = ctx.get_service("reth")
        seq = ctx.get_service("sequencer")
        # We just need to stop one bridge operator
        bridge_operator = ctx.get_service("bridge.0")
        # Kill the operator so the bridge does not process the DRT transaction
        bridge_operator.stop()
        self.logger.debug("Bridge operators stopped")

        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]

        rethrpc = reth.create_rpc()

        seqrpc = seq.create_rpc()

        self.logger.debug(f"BTC URL: {btc_url}")
        self.logger.debug(f"BTC user: {btc_user}")
        self.logger.debug(f"BTC password: {btc_password}")

        bridge_pk = get_bridge_pubkey(seqrpc)
        el_address = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        addr = get_address(0)
        refund_addr = get_address(1)
        recovery_addr = get_recovery_address(0, bridge_pk)

        # First let's see if the EVM side has no funds
        # Make sure that the el_address has zero balance
        balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        assert balance == 0, "EVM balance is not zero"

        # Also make sure that the recovery_address balance is also zero
        btc_balance = get_balance_recovery(
            recovery_addr,
            bridge_pk,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"DRT BTC balance (before DRT): {btc_balance}")
        assert btc_balance == 0, "BTC balance is not zero"

        # Generate Plenty of BTC to address for the DRT
        self.logger.debug(f"Generating 102 blocks to address: {addr}")
        btcrpc.proxy.generatetoaddress(102, addr)
        self.logger.debug(f"Generated 102 blocks to address: {addr}")
        time.sleep(0.5)

        # Generate Plenty of BTC to recovery address for the take back path
        self.logger.debug(f"Generating 102 blocks to recovery address: {recovery_addr}")
        btcrpc.proxy.generatetoaddress(102, recovery_addr)
        self.logger.debug(f"Generated 110 blocks to recovery address: {recovery_addr}")
        time.sleep(0.5)

        # DRT same block
        txid_drt = self.make_drt(ctx, el_address, bridge_pk, maturity=0)
        self.logger.debug(f"Deposit Request Transaction: {txid_drt}")
        time.sleep(0.5)

        # Now we need to generate a bunch of blocks
        # since they will be able to spend the DRT output.
        # We need to wait for the reclaim path 1008 blocks to mature
        # so that we can use the take back path to spend the DRT output.
        # Breaking by chunks
        chunks = 8
        blocks_to_generate = DEFAULT_TAKEBACK_TIMEOUT // chunks
        self.logger.debug(f"Generating {DEFAULT_TAKEBACK_TIMEOUT} blocks in {chunks} chunks")
        for i in range(chunks):
            self.logger.debug(f"Generating {blocks_to_generate} blocks in chunk {i+1}/{chunks}")
            btcrpc.proxy.generatetoaddress(blocks_to_generate, UNSPENDABLE_ADDRESS)

        # wait up a little bit
        time.sleep(0.25)

        # Make sure that the BTC refund address has the expected balance
        refund_btc_balance = get_balance(
            refund_addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"User BTC balance (before takeback): {refund_btc_balance}")
        assert refund_btc_balance == 0, "BTC balance is not zero"

        # Spend the take back path
        take_back_tx = bytes(
            take_back_transaction(refund_addr, bridge_pk, btc_url, btc_user, btc_password)
        ).hex()
        self.logger.debug("Take back tx generated")

        # Send the transaction to the Bitcoin network
        txid = btcrpc.proxy.sendrawtransaction(take_back_tx)
        self.logger.debug(f"sent take back tx with txid = {txid} for address {el_address}")
        btcrpc.proxy.generatetoaddress(2, UNSPENDABLE_ADDRESS)
        time.sleep(1)

        # Make sure that the BTC recovery address has 0 BTC
        btc_balance = get_balance_recovery(
            recovery_addr,
            bridge_pk,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"DRT BTC balance (after takeback): {btc_balance}")
        assert btc_balance == 0, "BTC balance is not zero"

        # Make sure that the BTC refund address has the expected balance
        refund_btc_balance = get_balance(
            refund_addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"User BTC balance (after takeback): {refund_btc_balance}")
        expected_balance_lower_bound = DEFAULT_ROLLUP_PARAMS["deposit_amount"] - 100_000_000
        assert refund_btc_balance >= expected_balance_lower_bound, "BTC balance is not as expected"

        # Now let's see if the EVM side has no funds
        # Make sure that the el_address has zero balance
        balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        self.logger.debug(f"EVM balance (after takeback): {balance}")
        assert balance == 0, "EVM balance is not zero"

        # Restart the bridge operator
        bridge_operator.start()

        return True

    def make_drt(self, ctx: flexitest.RunContext, el_address, musig_bridge_pk, maturity=0):
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

        self.logger.debug(f"BTC URL: {btc_url}")
        self.logger.debug(f"BTC user: {btc_user}")
        self.logger.debug(f"BTC password: {btc_password}")

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
        if maturity > 0:
            btcrpc.proxy.generatetoaddress(maturity, seq_addr)
            time.sleep(3)

        return txid
