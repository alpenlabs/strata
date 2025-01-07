import flexitest

import testenv
from utils import get_bridge_pubkey, wait_for_proof_with_time_out


@flexitest.register
class ProverEvmEeWithdrawl(testenv.BridgeTestBase):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        # Generate addresses
        withdraw_address = ctx.env.gen_ext_btc_address()
        el_address = self.eth_account.address

        bridge_pk = get_bridge_pubkey(self.seqrpc)
        self.debug(f"Bridge pubkey: {bridge_pk}")

        # make two deposits
        self.deposit(ctx, el_address, bridge_pk)
        self.deposit(ctx, el_address, bridge_pk)

        # Withdraw
        (_, tx_receipt, _) = self.withdraw(ctx, el_address, withdraw_address)
        block_id = tx_receipt["blockHash"].hex()

        block = rethrpc.eth_getBlockByHash(block_id, True)
        print(f"Got block: {block}")

        # Get the witness data
        witness_data = rethrpc.strataee_getBlockWitness(block_id, True)
        assert witness_data is not None, "non empty witness"
        # print(f"Got witness data: {witness_data}")

        print("Creating the proving task")
        task_ids = prover_client_rpc.dev_strata_proveElBlocks((block_id, block_id))
        print(f"got task ids: {task_ids}")
        task_id = task_ids[0]
        assert task_id is not None

        time_out = 10 * 60
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
