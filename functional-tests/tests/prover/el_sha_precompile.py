import time

import flexitest
from solcx import compile_source, install_solc, set_solc_version
from web3 import Web3

from utils import (
    wait_until_with_value,
    el_slot_to_block_id,
    wait_for_proof_with_time_out,
)
from envs import testenv


@flexitest.register
class ElShaPrecompileTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        install_solc(version="0.8.16")
        set_solc_version("0.8.16")
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")
        reth_rpc = reth.create_rpc()

        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        web3: Web3 = reth.create_web3()
        web3.eth.default_account = web3.address

        # Deploy the contract
        abi, bytecode = get_contract()
        contract = web3.eth.contract(abi=abi, bytecode=bytecode)
        tx_hash = contract.constructor().transact()
        tx_receipt = web3.eth.wait_for_transaction_receipt(tx_hash, timeout=30)

        # Call the updateHash contract method
        contract_instance = web3.eth.contract(abi=abi, address=tx_receipt.contractAddress)
        tx_hash = contract_instance.functions.updateHash().transact()
        tx_receipt = web3.eth.wait_for_transaction_receipt(tx_hash, timeout=30)

        # Prove the corresponding EE block
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

        wait_for_proof_with_time_out(prover_client_rpc, task_id, time_out=30)


def get_contract():
    compiled_sol = compile_source(
        """
        pragma solidity ^0.8.0;

        contract MessageHasher {
            string public message;
            bytes32 public messageHash;

            constructor() {
                message = "Hello";
                messageHash = sha256(abi.encodePacked(message));
            }

            // Updates the messageHash state variable with the hash of the current message.
            function updateHash() public {
                messageHash = sha256(abi.encodePacked(message));
            }
        }
        """,
        output_values=["abi", "bin"],
    )

    _, contract_interface = compiled_sol.popitem()
    bytecode = contract_interface["bin"]
    abi = contract_interface["abi"]
    return abi, bytecode
