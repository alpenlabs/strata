import flexitest
from web3 import Web3

from envs import testenv
from utils import el_slot_to_block_commitment, wait_for_proof_with_time_out, wait_until_with_value
from utils.schnorr import (
    get_precompile_input,
    get_test_schnnor_secret_key,
    make_schnorr_precompile_call,
)


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        web3: Web3 = reth.create_web3()

        # Wait for first EE block
        wait_until_with_value(
            lambda: web3.eth.get_block("latest")["number"],
            lambda block_height: block_height > 0,
            error_with="EE blocks not generated",
        )

        secret_key = get_test_schnnor_secret_key()
        msg = "AlpenStrata"
        precompile_input = get_precompile_input(secret_key, msg)
        txid, _data = make_schnorr_precompile_call(web3, precompile_input)

        txn = web3.eth.get_transaction(txid)
        block_number = txn.blockNumber

        # Parameters defining the range of Execution Engine (EE) blocks to be proven.
        ee_prover_params = {
            "start_block": block_number - 1,
            "end_block": block_number + 1,
        }

        # Wait for end EE block
        wait_until_with_value(
            lambda: web3.eth.get_block("latest")["number"],
            lambda block_height: block_height >= ee_prover_params["end_block"],
            error_with="EE blocks not generated",
        )

        # Dispatch the prover task
        start_block = el_slot_to_block_commitment(rethrpc, ee_prover_params["start_block"])
        end_block = el_slot_to_block_commitment(rethrpc, ee_prover_params["end_block"])

        task_ids = prover_client_rpc.dev_strata_proveElBlocks((start_block, end_block))
        self.debug(f"got task ids: {task_ids}")
        task_id = task_ids[0]
        self.debug(f"using task id: {task_id}")
        assert task_id is not None

        time_out = 30
        is_proof_generation_completed = wait_for_proof_with_time_out(
            prover_client_rpc, task_id, time_out=time_out
        )
        assert is_proof_generation_completed
