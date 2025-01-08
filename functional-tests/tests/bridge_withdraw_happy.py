import flexitest
from strata_utils import get_balance

from envs import net_settings, testenv
from envs.rollup_params_cfg import RollupConfig
from utils import utils

# Local constants
# Gas for the withdrawal transaction
WITHDRAWAL_GAS_FEE = 22_000  # technically is 21_000
# Ethereum Private Key
# NOTE: don't use this private key in production
ETH_PRIVATE_KEY = "0x0000000000000000000000000000000000000000000000000000000000000001"


@flexitest.register
class BridgeWithdrawHappyTest(testenv.BridgeTestBase):
    """
    Makes two DRT deposits to the same EL address, then makes a withdrawal to a change address.

    Checks if the balance of the EL address is expected
    and if the BTC balance of the change address is expected.
    """

    def __init__(self, ctx: flexitest.InitContext):
        fast_batch_settings = net_settings.get_fast_batch_settings()
        ctx.set_env(
            testenv.BasicEnvConfig(pre_generate_blocks=101, rollup_settings=fast_batch_settings)
        )

    def main(self, ctx: flexitest.RunContext):
        # Generate addresses
        address = ctx.env.gen_ext_btc_address()
        withdraw_address = ctx.env.gen_ext_btc_address()
        el_address = self.eth_account.address

        self.debug(f"Address: {address}")
        self.debug(f"Change Address: {withdraw_address}")
        self.debug(f"EL Address: {el_address}")

        cfg: RollupConfig = ctx.env.rollup_cfg()
        # D BTC
        deposit_amount = cfg.deposit_amount
        # BTC Operator's fee for withdrawal
        operator_fee = cfg.operator_fee
        # BTC extra fee for withdrawal
        withdraw_extra_fee = cfg.withdraw_extra_fee

        # Original BTC balance
        btc_url = self.btcrpc.base_url
        btc_user = self.btc.get_prop("rpc_user")
        btc_password = self.btc.get_prop("rpc_password")
        original_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
        self.debug(f"BTC balance before withdraw: {original_balance}")

        # Make sure starting ETH balance is 0
        check_initial_eth_balance(self.rethrpc, el_address, self.debug)

        bridge_pk = utils.get_bridge_pubkey(self.seqrpc)
        self.debug(f"Bridge pubkey: {bridge_pk}")

        # make two deposits
        self.deposit(ctx, el_address, bridge_pk)
        self.deposit(ctx, el_address, bridge_pk)

        # Withdraw
        self.withdraw(ctx, el_address, withdraw_address)

        # Confirm BTC side
        # We expect final BTC balance to be D BTC minus operator fees
        difference = deposit_amount - operator_fee - withdraw_extra_fee
        confirm_btc_withdrawal(
            withdraw_address,
            btc_url,
            btc_user,
            btc_password,
            original_balance,
            difference,
            self.debug,
        )

        return True


def check_initial_eth_balance(rethrpc, address, debug_fn=print):
    """Asserts that the initial ETH balance for `address` is zero."""
    balance = int(rethrpc.eth_getBalance(address), 16)
    debug_fn(f"Strata Balance before deposits: {balance}")
    assert balance == 0, "Strata balance is not expected (should be zero initially)"


def confirm_btc_withdrawal(
    withdraw_address,
    btc_url,
    btc_user,
    btc_password,
    original_balance,
    expected_increase,
    debug_fn=print,
):
    """
    Wait for the BTC balance to reflect the withdrawal and confirm the final balance
    equals `original_balance + expected_increase`.
    """
    # Wait for the new balance,
    # this includes waiting for a new batch checkpoint,
    # duty processing by the bridge clients and maturity of the withdrawal.
    utils.wait_until(
        lambda: get_balance(withdraw_address, btc_url, btc_user, btc_password) > original_balance,
        timeout=60,
    )

    # Check final BTC balance
    btc_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
    debug_fn(f"BTC final balance: {btc_balance}")
    debug_fn(f"Expected final balance: {original_balance + expected_increase}")

    assert (
        btc_balance == original_balance + expected_increase
    ), "BTC balance after withdrawal is not as expected"
