import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import (
    deposit_request_transaction,
    get_balance,
    get_balance_recovery,
    take_back_transaction,
)

from envs import testenv
from utils import get_bridge_pubkey, wait_until
from utils.constants import DEFAULT_TAKEBACK_TIMEOUT, UNSPENDABLE_ADDRESS

# Local constants
# Fee for the take back path at 2 sat/vbyte
TAKE_BACK_FEE = 17_243


@flexitest.register
class BridgeDepositReclaimTest(testenv.StrataTester):
    """
    A test class for reclaim path scenarios of bridge deposits.

    It tests the functionality of broadcasting a deposit request transaction (DRT) for the bridge
    and verifying that the reclaim path works as expected.

    In the test we stop one of the bridge operators to prevent the DRT from being processed.
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(testenv.BasicEnvConfig(101))

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        reth = ctx.get_service("reth")
        seq = ctx.get_service("sequencer")
        # We just need to stop one bridge operator
        bridge_operator = ctx.get_service("bridge.0")
        # Kill the operator so the bridge does not process the DRT transaction
        bridge_operator.stop()
        self.debug("Bridge operators stopped")

        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]

        rethrpc = reth.create_rpc()

        seqrpc = seq.create_rpc()

        self.debug(f"BTC URL: {btc_url}")
        self.debug(f"BTC user: {btc_user}")
        self.debug(f"BTC password: {btc_password}")

        bridge_pk = get_bridge_pubkey(seqrpc)
        el_address = ctx.env.gen_el_address()
        user_addr = ctx.env.gen_ext_btc_address()
        refund_addr = ctx.env.gen_ext_btc_address()
        recovery_addr = ctx.env.gen_rec_btc_address()
        self.debug(f"El address: {el_address}")
        self.debug(f"User address: {user_addr}")
        self.debug(f"Refund address: {refund_addr}")
        self.debug(f"Recovery address: {recovery_addr}")

        # First let's see if the EVM side has no funds
        # Make sure that the el_address has zero balance
        balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        assert balance == 0, "EVM balance is not zero"

        # Make sure that the BTC refund address has the expected balance
        initial_refund_btc_balance = get_balance(
            refund_addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.debug(f"Initial refund BTC balance: {initial_refund_btc_balance}")

        # DRT same block
        txid_drt = self.make_drt(ctx, el_address, bridge_pk)
        self.debug(f"Deposit Request Transaction: {txid_drt}")

        # Now we need to generate a bunch of blocks
        # since they will be able to spend the DRT output.
        # We need to wait for the reclaim path 1008 blocks to mature
        # so that we can use the take back path to spend the DRT output.
        # Breaking by chunks
        chunks = 8
        blocks_to_generate = DEFAULT_TAKEBACK_TIMEOUT // chunks
        self.debug(f"Generating {DEFAULT_TAKEBACK_TIMEOUT} blocks in {chunks} chunks")
        for i in range(chunks):
            self.debug(f"Generating {blocks_to_generate} blocks in chunk {i + 1}/{chunks}")
            btcrpc.proxy.generatetoaddress(blocks_to_generate, UNSPENDABLE_ADDRESS)

        # Make sure that the BTC refund address has the expected balance
        wait_until(
            lambda: get_balance(refund_addr, btc_url, btc_user, btc_password)
            == initial_refund_btc_balance
        )
        refund_btc_balance = get_balance(
            refund_addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.debug(f"User BTC balance (before takeback): {refund_btc_balance}")
        assert refund_btc_balance == initial_refund_btc_balance, "BTC balance has changed"

        # Spend the take back path
        take_back_tx = bytes(
            take_back_transaction(refund_addr, bridge_pk, btc_url, btc_user, btc_password)
        ).hex()
        self.debug("Take back tx generated")

        # Send the transaction to the Bitcoin network
        original_recovery_balance = get_balance_recovery(
            recovery_addr,
            bridge_pk,
            btc_url,
            btc_user,
            btc_password,
        )
        txid = btcrpc.proxy.sendrawtransaction(take_back_tx)
        self.debug(f"sent take back tx with txid = {txid} for address {el_address}")
        btcrpc.proxy.generatetoaddress(2, UNSPENDABLE_ADDRESS)
        wait_until(
            lambda: get_balance_recovery(recovery_addr, bridge_pk, btc_url, btc_user, btc_password)
            < original_recovery_balance
        )

        # Make sure that the BTC recovery address has 0 BTC
        btc_balance = get_balance_recovery(
            recovery_addr,
            bridge_pk,
            btc_url,
            btc_user,
            btc_password,
        )
        self.debug(f"DRT BTC balance (after takeback): {btc_balance}")
        assert btc_balance == 0, "BTC balance is not zero"

        # Make sure that the BTC refund address has the expected balance
        refund_btc_balance = get_balance(
            refund_addr,
            btc_url,
            btc_user,
            btc_password,
        )
        self.debug(f"User BTC balance (after takeback): {refund_btc_balance}")
        deposit_amount = ctx.env.rollup_cfg().deposit_amount
        expected_balance = 5 * deposit_amount - TAKE_BACK_FEE
        assert refund_btc_balance >= expected_balance, "BTC balance is not as expected"

        # Now let's see if the EVM side has no funds
        # Make sure that the el_address has zero balance
        balance = int(rethrpc.eth_getBalance(f"0x{el_address}"), 16)
        self.debug(f"EVM balance (after takeback): {balance}")
        assert balance == 0, "EVM balance is not zero"

        # Restart the bridge operator
        bridge_operator.start()

        return True

    def make_drt(self, ctx: flexitest.RunContext, el_address, musig_bridge_pk):
        """
        Deposit Request Transaction
        """
        # Get relevant data
        btc = ctx.get_service("bitcoin")
        btcrpc: BitcoindClient = btc.create_rpc()
        btc_url = btcrpc.base_url
        btc_user = btc.props["rpc_user"]
        btc_password = btc.props["rpc_password"]

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

        return txid
