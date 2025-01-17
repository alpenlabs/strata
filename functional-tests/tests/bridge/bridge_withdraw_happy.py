import flexitest
from strata_utils import extract_p2tr_pubkey, get_balance, xonlypk_to_descriptor

from envs import net_settings, testenv
from envs.rollup_params_cfg import RollupConfig
from mixins import bridge_mixin
from utils import confirm_btc_withdrawal, get_bridge_pubkey


@flexitest.register
class BridgeWithdrawHappyTest(bridge_mixin.BridgeMixin):
    """
    Makes two DRT deposits to the same EL address, then makes a withdrawal to a change address.

    Checks if the balance of the EL address is expected
    and if the BTC balance of the change address is expected.

    NOTE: The withdrawal destination is a Bitcoin Output Script Descriptor (BOSD).
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
        bridge_pk = get_bridge_pubkey(self.seqrpc)
        self.debug(f"Bridge pubkey: {bridge_pk}")

        original_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
        self.debug(f"BTC balance before deposit: {original_balance}")

        # Make sure starting ETH balance is 0
        check_initial_eth_balance(self.rethrpc, el_address, self.debug)

        # Perform two deposits
        self.deposit(ctx, el_address, bridge_pk)
        self.deposit(ctx, el_address, bridge_pk)
        original_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
        self.debug(f"BTC balance after deposit: {original_balance}")

        # withdraw
        xonlypk = extract_p2tr_pubkey(withdraw_address)
        self.debug(f"XOnly PK: {xonlypk}")
        bosd = xonlypk_to_descriptor(xonlypk)
        self.debug(f"BOSD: {bosd}")
        _, withdraw_tx_receipt, _ = self.withdraw(ctx, el_address, bosd)

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
