import flexitest

from envs import testenv
from envs.rollup_params_cfg import RollupConfig
from utils import check_sequencer_down, get_bridge_pubkey, wait_until, wait_until_with_value
from utils.constants import SATS_TO_WEI


@flexitest.register
class BridgeDepositSequencerUnreliableTest(testenv.BridgeTestBase):
    """
    Makes two DRT deposits to the same EL address
    After the first DRT is processed and EL address has balance,the sequencer is
    restarted . After restarting check if EL address has required funds
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        address = ctx.env.gen_ext_btc_address()
        withdraw_address = ctx.env.gen_ext_btc_address()
        el_address = self.eth_account.address
        bridge_pk = get_bridge_pubkey(self.seqrpc)
        self.debug(f"Address: {address}")
        self.debug(f"Change Address: {withdraw_address}")
        self.debug(f"EL address: {el_address}")
        self.debug(f"Bridge pubkey: {bridge_pk}")

        cfg: RollupConfig = ctx.env.rollup_cfg()

        # deposit
        self.deposit(ctx, el_address, bridge_pk)
        # stop sequencer
        self.seq.stop()

        # wait until sequencer stops
        wait_until(lambda: check_sequencer_down(self.seqrpc))

        self.debug("Making DRT")
        # make deposit request transaction
        self.make_drt(ctx, el_address, bridge_pk)

        # start again
        self.seq.start()

        wait_until(
            lambda: not check_sequencer_down(self.seqrpc),
            error_with="Sequencer did not start on time",
            timeout=10,
        )

        balance_after_deposits = wait_until_with_value(
            lambda: int(self.rethrpc.eth_getBalance(el_address), 16),
            predicate=lambda v: v == 2 * cfg.deposit_amount * SATS_TO_WEI,
            timeout=600,
        )
        self.debug(f"Strata Balance after deposits: {balance_after_deposits}")

        return True
