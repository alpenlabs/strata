from web3 import Web3
import flexitest

from envs import testenv
from utils import (
    el_slot_to_block_id,
    wait_for_proof_with_time_out,
    wait_until_with_value,
)
from utils.eth import make_token_transfer

PROOF_TIMEOUT = 30


@flexitest.register
class ProverClientTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        reth = ctx.get_service("reth")
        reth_rpc = reth.create_rpc()
        web3: Web3 = reth.create_web3()

        # Wait until at least one EE block is generated.
        wait_until_with_value(
            lambda: web3.eth.get_block("latest")["number"],
            lambda height: height > 0,
            error_with="EE blocks not generated",
        )

        transfer_amount = 1_000_000_000_000_000_000
        recipient = "5400000000000000000000000000000000000011"
        tx_receipt = make_token_transfer(web3, transfer_amount, recipient)

        ee_prover_params = {
            "start_block": tx_receipt["blockNumber"] - 1,
            "end_block": tx_receipt["blockNumber"] + 1,
        }

        # Wait until the end EE block is generated.
        wait_until_with_value(
            lambda: web3.eth.get_block("latest")["number"],
            lambda height: height >= ee_prover_params["end_block"],
            error_with="EE blocks not generated",
        )

        start_block_id = el_slot_to_block_id(reth_rpc, ee_prover_params["start_block"])
        end_block_id = el_slot_to_block_id(reth_rpc, ee_prover_params["end_block"])

        task_ids = prover_client_rpc.dev_strata_proveElBlocks((start_block_id, end_block_id))
        self.debug(f"Prover task IDs received: {task_ids}")

        if not task_ids:
            raise Exception("No task IDs received from prover_client_rpc")

        task_id = task_ids[0]
        self.debug(f"Using task ID: {task_id}")

        time_out = 30
        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=time_out)
