import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from strata_utils import extract_p2tr_pubkey, get_balance, xonlypk_to_descriptor

from envs.rollup_params_cfg import RollupConfig
from mixins import bridge_mixin
from utils import (
    confirm_btc_withdrawal,
    get_bridge_pubkey,
    wait_for_proof_with_time_out,
    wait_until,
)


@flexitest.register
class ProverDepositWithdrawTest(bridge_mixin.BridgeMixin):
    """
    Checks that the prover is able to prove the checkpoint that contains
    deposit and withdrawal transactions.

    Since withdrawal can't currently happen without a deposit, those two
    (semantically different) tests are merged in one.

    NOTE: The withdrawal destination is a Bitcoin Output Script Descriptor (BOSD).
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")
        # random big checkpoint to not interfere with other tests in the prover env.
        self._chkpt_id = 339179

    def main(self, ctx: flexitest.RunContext):
        self.warning("SKIPPING TEST prover_el_deposit_withdraw")
        return True

        evm_addr = self.eth_account.address
        bridge_pk = get_bridge_pubkey(self.seqrpc)

        # Init RPCs.
        btc = ctx.get_service("bitcoin")
        btcrpc: BitcoindClient = btc.create_rpc()
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()
        # Wait some time until the prover client has loaded the ELFs and ready to accept RPCs.
        # It might happen this is already unnecessary.
        # TODO: Ideally, this should be an health check RPC to the prover client.
        time.sleep(5)

        # DEPOSIT part of the test
        # ------------------------

        # Do deposit on the L1.
        # Fix the strata block first (to optimize the search).
        start_block = int(rethrpc.eth_blockNumber(), base=16)
        l1_deposit_txn_id = self.deposit(ctx, evm_addr, bridge_pk)
        # Do twice the deposit, so the withdrawal will have funds for the gas.
        _ = self.deposit(ctx, evm_addr, bridge_pk)

        # Collect the L1 and L2 blocks where the deposit transaction was included.
        l1_deposit_tx_info = btcrpc.proxy.getrawtransaction(l1_deposit_txn_id, 1)
        l1_deposit_blockhash = l1_deposit_tx_info["blockhash"]
        l1_deposit_block_height = btcrpc.proxy.getblock(l1_deposit_blockhash, 1)["height"]
        self.info(f"deposit block height on L1: {l1_deposit_block_height}")

        l2_deposit_block_num = None
        end_block = int(rethrpc.eth_blockNumber(), base=16)
        for block_num in range(start_block, end_block + 1):
            block = rethrpc.eth_getBlockByNumber(hex(block_num), True)
            # Bridge-ins are currently handled as withdrawals in the block payload.
            withdrawals = block.get("withdrawals", None)
            if withdrawals is not None and len(withdrawals) != 0:
                l2_deposit_block_num = block_num
        self.info(f"deposit block num on L2: {l2_deposit_block_num}")

        # Proving
        self.test_checkpoint(
            l1_deposit_block_height, l2_deposit_block_num, prover_client_rpc, rethrpc
        )

        # Deposit is OK.
        # WITHDRAWAL part of the test.
        # ------------------------

        withdraw_address = ctx.env.gen_ext_btc_address()

        xonlypk = extract_p2tr_pubkey(withdraw_address)
        self.debug(f"XOnly PK: {xonlypk}")
        bosd = xonlypk_to_descriptor(xonlypk)
        self.debug(f"BOSD: {bosd}")

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

        # Withdraw
        _, withdraw_tx_receipt, _ = self.withdraw(ctx, evm_addr, bosd)

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

        # Collect L2 and L1 blocks where the withdrawal has happened.
        l2_withdraw_block_num = withdraw_tx_receipt["blockNumber"]
        self.info(f"withdrawal block num on L2: {l2_withdraw_block_num}")

        last_block_hash = btcrpc.proxy.getblockchaininfo()["bestblockhash"]
        last_block = btcrpc.proxy.getblock(last_block_hash, 1)
        # Check all blocks down from the latest.
        # Those blocks will have only coinbase tx for all the empty blocks.
        # Block with the withdrawal transfer will have at least two transactions.
        while len(last_block["tx"]) <= 1:
            last_block = btcrpc.proxy.getblock(last_block["previousblockhash"], 1)
        l1_withdraw_block_height = last_block["height"]
        self.info(f"withdrawal block height on L1: {l1_withdraw_block_height}")

        # Proving
        self.test_checkpoint(
            l1_withdraw_block_height, l2_withdraw_block_num, prover_client_rpc, rethrpc
        )

    def test_checkpoint(self, l1_block, l2_block, prover_client_rpc, rethrpc):
        self._chkpt_id += 1
        l1 = (l1_block - 1, l1_block + 1)
        l2 = (l2_block - 1, l2_block + 1)
        # Wait some time so the future blocks in the batches are finalized.
        # Given that L1 blocks are happening more frequent that L2, it's safe
        # to assert only L2 latest block.
        wait_until(
            lambda: int(rethrpc.eth_blockNumber(), base=16) > l2[1],
            timeout=60,
        )

        task_ids = prover_client_rpc.dev_strata_proveCheckpointRaw(self._chkpt_id, l1, l2)

        self.debug(f"got task ids: {task_ids}")
        task_id = task_ids[0]
        self.debug(f"using task id: {task_id}")
        assert task_id is not None

        is_proof_generation_completed = wait_for_proof_with_time_out(
            prover_client_rpc, task_id, time_out=30
        )
        assert is_proof_generation_completed
