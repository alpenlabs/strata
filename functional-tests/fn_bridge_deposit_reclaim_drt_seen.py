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

from constants import (
    DEFAULT_ROLLUP_PARAMS,
    DEFAULT_TAKEBACK_TIMEOUT,
    UNSPENDABLE_ADDRESS,
)
from entry import BasicEnvConfig
from utils import get_bridge_pubkey, get_logger, wait_until

# Local constants
# D BTC
DEPOSIT_AMOUNT = DEFAULT_ROLLUP_PARAMS["deposit_amount"]
# Fee for the take back path at 2 sat/vbyte
TAKE_BACK_FEE = 17_243


@flexitest.register
class BridgeDepositReclaimDrtSeenTest(flexitest.Test):
    """
    Tests the functionality of broadcasting a deposit request transaction (DRT) for the bridge
    and verifying that the reclaim path works as expected.

    In the test we stop one of the bridge operators to prevent the DRT from being processed.
    However, we let the operator be aware of the DRT before stopping it
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(
            BasicEnvConfig(
                pre_generate_blocks=101,
                auto_generate_blocks=False,
                # Set a high message interval to make sure the DRT is not processed.
                # This is 10 minutes.
                # NOTE: Alternatively, we could take one bridge operator down
                #       to prevent the DRT from being processed.
                message_interval=int(10 * 60 * 1_000),
            )
        )
        self.logger = get_logger("BridgeDepositReclaimDrtSeenTest")

    def main(self, ctx: flexitest.RunContext):
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

        bridge_pk = get_bridge_pubkey(seqrpc)
        self.logger.debug(f"Bridge pubkey: {bridge_pk}")
        el_address = "deadf001900dca3ebeefdeadf001900dca3ebeef"
        self.logger.debug(f"EL address: {el_address}")
        addr = get_address(0)
        self.logger.debug(f"Address: {addr}")
        refund_addr = get_address(1)
        self.logger.debug(f"Refund address: {refund_addr}")
        recovery_addr = get_recovery_address(0, bridge_pk)
        self.logger.debug(f"Recovery Address: {recovery_addr}")

        seq_addr = seq.get_prop("address")
        self.logger.debug(f"Sequencer Address: {seq_addr}")

        # First let's see if the EVM side has no funds
        # Make sure that the el_address has zero balance
        balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        assert balance == 0, "EVM balance is not zero"

        # Also make sure that the all addresses have zero balance
        btc_addr_balance = get_balance(
            addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"Address BTC balance (before DRT): {btc_addr_balance}")
        assert btc_addr_balance == 0, "Address BTC balance is not zero"
        btc_refund_balance = get_balance(
            refund_addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"Refund BTC balance (before DRT): {btc_refund_balance}")
        assert btc_refund_balance == 0, "Refund BTC balance is not zero"
        btc_recovery_balance = get_balance_recovery(
            recovery_addr,
            bridge_pk,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"DRT BTC balance (before DRT): {btc_recovery_balance}")
        assert btc_recovery_balance == 0, "Recovery BTC balance is not zero"

        # Generate Plenty of BTC to address for the DRT
        self.logger.debug(f"Generating 102 blocks to address: {addr}")
        btcrpc.proxy.generatetoaddress(102, addr)
        self.logger.debug(f"Generated 102 blocks to address: {addr}")
        wait_until(lambda: get_balance(addr, btc_url, btc_user, btc_password) > 0)

        # Generate Plenty of BTC to recovery address for the take back path
        self.logger.debug(f"Generating 102 blocks to recovery address: {recovery_addr}")
        btcrpc.proxy.generatetoaddress(102, recovery_addr)
        self.logger.debug(f"Generated 110 blocks to recovery address: {recovery_addr}")
        wait_until(
            lambda: get_balance_recovery(recovery_addr, bridge_pk, btc_url, btc_user, btc_password)
            > 0
        )
        balance_recovery = get_balance_recovery(
            recovery_addr, bridge_pk, btc_url, btc_user, btc_password
        )
        self.logger.debug(f"Recovery BTC balance: {balance_recovery}")
        assert balance_recovery > 0, "Recovery BTC balance is not positive"

        # DRT same block
        txid_drt = self.make_drt(ctx, el_address, bridge_pk)
        self.logger.debug(f"Deposit Request Transaction: {txid_drt}")
        time.sleep(1)

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
        time.sleep(1)

        # Make sure that the BTC refund address has the expected balance
        refund_btc_balance = get_balance(
            refund_addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"Refund BTC balance (before takeback): {refund_btc_balance}")
        assert refund_btc_balance == 0, "Refund BTC balance is not zero"

        # Spend the take back path
        take_back_tx = bytes(
            take_back_transaction(refund_addr, bridge_pk, btc_url, btc_user, btc_password)
        ).hex()
        self.logger.debug("Take back tx generated")

        # Send the transaction to the Bitcoin network
        txid = btcrpc.proxy.sendrawtransaction(take_back_tx)
        self.logger.debug(f"sent take back tx with txid = {txid} for address {el_address}")
        btcrpc.proxy.generatetoaddress(2, UNSPENDABLE_ADDRESS)
        wait_until(lambda: get_balance(refund_addr, btc_url, btc_user, btc_password) > 0)

        # Make sure that the BTC refund address has the expected balance
        refund_btc_balance_post = get_balance(
            refund_addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.logger.debug(f"Refund BTC balance (after takeback): {refund_btc_balance_post}")

        assert (
            refund_btc_balance_post == balance_recovery - TAKE_BACK_FEE
        ), "Refund BTC balance is not expected"

        # Get the balance of the EL address after the deposits
        balance_post = int(rethrpc.eth_getBalance(el_address), 16)
        self.logger.debug(f"Strata Balance after deposits: {balance_post}")
        assert balance_post == 0, "Strata balance is not zero"

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
