import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

from envs import testenv
from utils import get_bridge_pubkey, wait_for_proof_with_time_out


@flexitest.register
class ProverDepositTest(testenv.BridgeTestBase):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        evm_addr = "deedf001900dca3ebeefdeadf001900dca3ebeef"

        btc = ctx.get_service("bitcoin")
        btcrpc: BitcoindClient = btc.create_rpc()
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        # Do deposit on the L1.
        # Fix the strata block first.
        start_block = int(rethrpc.eth_blockNumber(), base=16)
        l1_deposit_txn_id = self.deposit(ctx, evm_addr, get_bridge_pubkey(self.seqrpc))

        # Collect the L1 and L2 blocks where the deposit transaction was included.
        l1_deposit_tx_info = btcrpc.proxy.getrawtransaction(l1_deposit_txn_id, 1)
        l1_deposit_blockhash = l1_deposit_tx_info["blockhash"]
        l1_deposit_block_height = btcrpc.proxy.getblock(l1_deposit_blockhash, 1)["height"]
        print(f"deposit block height on L1: {l1_deposit_block_height}")

        l2_deposit_block_num = None
        end_block = int(rethrpc.eth_blockNumber(), base=16)
        for block_num in range(start_block, end_block + 1):
            block = rethrpc.eth_getBlockByNumber(hex(block_num), True)
            # Bridge-ins are currently handled as withdrawals in the block payload.
            withdrawals = block.get("withdrawals", None)
            if withdrawals is not None and len(withdrawals) != 0:
                l2_deposit_block_num = block_num
        print(f"deposit block num on L2: {l2_deposit_block_num}")

        # Init the prover client
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()
        time.sleep(5)

        # Proving task with with few L1 and L2 blocks including the deposit transaction
        l1_range = (l1_deposit_block_height - 1, l1_deposit_block_height + 1)
        l2_range = (l2_deposit_block_num - 1, l2_deposit_block_num + 1)
        # 339179 is a random checkpoint index.
        # Chosen to not collide with other checkpoint tests in the same prover env.
        task_ids = prover_client_rpc.dev_strata_proveCheckpointRaw(339179, l1_range, l2_range)

        self.debug(f"got task ids: {task_ids}")
        task_id = task_ids[0]
        self.debug(f"using task id: {task_id}")
        assert task_id is not None

        time_out = 30
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
