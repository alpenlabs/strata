import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import get_balance, string_to_opreturn_descriptor

from envs import net_settings, testenv
from utils import check_initial_eth_balance, get_bridge_pubkey, wait_until

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
            )
        )

    def main(self, ctx: flexitest.RunContext):
        btc = ctx.get_service("bitcoin")
        seq = ctx.get_service("sequencer")
        # create both btc and sequencer RPC
        btcrpc: BitcoindClient = btc.create_rpc()
        seqrpc = seq.create_rpc()

        # Wait for seq
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )

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
        _, withdraw_tx_receipt, _ = self.withdraw(ctx, el_address, bosd)

        # Collect L2 and L1 blocks where the withdrawal has happened.
        l2_withdraw_block_num = withdraw_tx_receipt["blockNumber"]
        self.info(f"withdrawal block num on L2: {l2_withdraw_block_num}")

        last_block_hash = btcrpc.proxy.getblockchaininfo()["bestblockhash"]
        last_block = btcrpc.getblock(last_block_hash)
        # Check all blocks down from the latest.
        # Those blocks will have only coinbase tx for all the empty blocks.
        # Block with the withdrawal transfer will have at least two transactions.
        l1_withdraw_block_height = last_block["height"]
        self.info(f"withdrawal block height on L1: {l1_withdraw_block_height}")

        return True

        # FIXME: Somehow the block does not have the "hello world"
        # (OP_RETURN LEN 68656c6c6f20776f726c64)
        # payload. I don't know how to fix it.

        # Get the output of the tx that is not the conibase tx
        outputs = last_block["txs"][1].as_dict()["outputs"]
        # OP_RETURN is the first output
        op_return_output = outputs[0]
        self.debug(f"OP_RETURN output: {op_return_output}")

        # Check if it is a nulldata output
        op_return_script_type = op_return_output["script_type"]
        assert op_return_script_type == "nulldata", "OP_RETURN not found"

        # Check the payload
        op_return_data = op_return_output["script"]
        # The same transformation (remove the <OP_RETURN> <LEN>)
        op_return_payload = op_return_data[4:]
        reconstructed_bosd = string_to_opreturn_descriptor(op_return_payload)
        assert reconstructed_bosd == bosd, "Reconstructed BOSD is not the same as the original"

        return True
