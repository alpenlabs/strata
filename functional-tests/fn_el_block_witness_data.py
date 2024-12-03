import logging
import time
from pathlib import Path

import flexitest
from solcx import compile_source, install_solc, set_solc_version
from web3 import Web3


@flexitest.register
class ElBlockWitnessDataGenerationTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        install_solc(version="0.8.16")
        set_solc_version("0.8.16")
        ctx.set_env("basic")
        self.logger = logging.getLogger(Path(__file__).stem)

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")
        rethrpc = reth.create_rpc()

        web3: Web3 = reth.create_web3()
        web3.eth.default_account = web3.address

        # Deploy the contract
        abi, bytecode = get_contract()
        contract = web3.eth.contract(abi=abi, bytecode=bytecode)
        tx_hash = contract.constructor().transact()
        tx_receipt = web3.eth.wait_for_transaction_receipt(tx_hash, timeout=30)

        # Get the block hash where contract was deployed
        assert tx_receipt["status"] == 1
        blocknum = tx_receipt.blockNumber
        blockhash = rethrpc.eth_getBlockByNumber(hex(blocknum), False)["hash"]

        # wait for witness data generation
        time.sleep(1)

        # Get the witness data
        witness_data = rethrpc.strataee_getBlockWitness(blockhash, True)
        assert witness_data is not None, "non empty witness"

        self.logger.debug(witness_data)


def get_contract():
    compiled_sol = compile_source(
        """
        pragma solidity ^0.8.0;

        contract Greeter {
            string public greeting;

            constructor() public {
                greeting = 'Hello';
            }
        }
        """,
        output_values=["abi", "bin"],
    )

    _, contract_interface = compiled_sol.popitem()
    bytecode = contract_interface["bin"]
    abi = contract_interface["abi"]
    return abi, bytecode
