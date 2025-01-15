import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import get_balance, string_to_opreturn_descriptor

from envs import net_settings, testenv
from utils import generate_n_blocks, get_bridge_pubkey, wait_until

# Local constants
# Gas for the withdrawal transaction
WITHDRAWAL_GAS_FEE = 22_000  # technically is 21_000
# Ethereum Private Key
# NOTE: don't use this private key in production
ETH_PRIVATE_KEY = "0x0000000000000000000000000000000000000000000000000000000000000001"


@flexitest.register
class BridgeWithdrawHappyOpReturnTest(testenv.BridgeTestBase):
    """
    Makes two DRT deposits to the same EL address, then makes a withdrawal to an OP_RETURN.

    Checks if the balance of the EL address is expected
    and if the BTC has an OP_RETURN block.
    """

    def __init__(self, ctx: flexitest.InitContext):
        fast_batch_settings = net_settings.get_fast_batch_settings()
        ctx.set_env(
            testenv.BasicEnvConfig(
                pre_generate_blocks=101,
                rollup_settings=fast_batch_settings,
                # need to manually control the block generations
                auto_generate_blocks=False,
            )
        )

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        # create both btc and sequencer RPC
        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()
        # generate 5 btc blocks
        generate_n_blocks(btcrpc, 5)

        # Wait for seq
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )
        generate_n_blocks(btcrpc, 5)

        # Generate addresses
        address = ctx.env.gen_ext_btc_address()
        withdraw_address = ctx.env.gen_ext_btc_address()
        el_address = self.eth_account.address
        payload = "hello world"
        bosd = string_to_opreturn_descriptor(payload)
        self.debug(f"BOSD: {bosd}")

        self.debug(f"Address: {address}")
        self.debug(f"Change Address: {withdraw_address}")
        self.debug(f"EL Address: {el_address}")

        # Original BTC balance
        btc_url = self.btcrpc.base_url
        btc_user = self.btc.get_prop("rpc_user")
        btc_password = self.btc.get_prop("rpc_password")
        original_balance = get_balance(withdraw_address, btc_url, btc_user, btc_password)
        self.debug(f"BTC balance before withdraw: {original_balance}")

        # Make sure starting ETH balance is 0
        check_initial_eth_balance(self.rethrpc, el_address, self.debug)

        bridge_pk = get_bridge_pubkey(self.seqrpc)
        self.debug(f"Bridge pubkey: {bridge_pk}")

        # make two deposits
        self.deposit(ctx, el_address, bridge_pk)
        self.deposit(ctx, el_address, bridge_pk)

        # Withdraw
        self.withdraw_op_return(ctx, el_address, bosd)

        # Move forward a single block
        block = generate_n_blocks(btcrpc, 1)[0]  # There's only one
        last_block = btcrpc.getblock(block)

        # Get the output of the tx
        # OP_RETURN is the second output
        outputs = last_block["txs"][0].as_dict()["outputs"]
        op_return_output = outputs[1]
        self.debug(f"OP_RETURN output: {op_return_output}")
        op_return_script_type = op_return_output["script_type"]
        assert op_return_script_type == "nulldata", "OP_RETURN not found"

        return True


def check_initial_eth_balance(rethrpc, address, debug_fn=print):
    """Asserts that the initial ETH balance for `address` is zero."""
    balance = int(rethrpc.eth_getBalance(address), 16)
    debug_fn(f"Strata Balance before deposits: {balance}")
    assert balance == 0, "Strata balance is not expected (should be zero initially)"
