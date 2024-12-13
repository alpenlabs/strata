import time

import flexitest
from web3 import Web3

from entry import TestStrata


@flexitest.register
class ElBalanceTransferTest(TestStrata):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")

        web3: Web3 = reth.create_web3()

        source = web3.address
        dest = web3.to_checksum_address("0x0000000000000000000000000000000000000001")
        basefee_address = web3.to_checksum_address("5400000000000000000000000000000000000010")
        beneficiary_address = web3.to_checksum_address("5400000000000000000000000000000000000011")

        self.debug(f"{web3.is_connected()}")
        original_block_no = web3.eth.block_number
        dest_original_balance = web3.eth.get_balance(dest)
        source_original_balance = web3.eth.get_balance(source)
        basefee_original_balance = web3.eth.get_balance(basefee_address)
        beneficiary_original_balance = web3.eth.get_balance(beneficiary_address)

        self.debug(f"{original_block_no}, {dest_original_balance}")

        to_transfer = 1_000_000_000_000_000_000

        web3.eth.send_transaction(
            {"to": dest, "value": hex(to_transfer), "gas": hex(100000), "from": source}
        )

        time.sleep(2)

        final_block_no = web3.eth.block_number
        dest_final_balance = web3.eth.get_balance(dest)
        source_final_balance = web3.eth.get_balance(source)
        basefee_final_balance = web3.eth.get_balance(basefee_address)
        beneficiary_final_balance = web3.eth.get_balance(beneficiary_address)

        self.debug(f"{final_block_no}, {dest_final_balance}")

        assert original_block_no < final_block_no
        assert dest_original_balance + to_transfer == dest_final_balance

        basefee_balance_change = basefee_final_balance - basefee_original_balance
        assert basefee_balance_change > 0
        beneficiary_balance_change = beneficiary_final_balance - beneficiary_original_balance
        assert beneficiary_balance_change > 0
        source_balance_change = source_final_balance - source_original_balance
        assert (
            source_balance_change
            + basefee_balance_change
            + beneficiary_balance_change
            + to_transfer
            == 0
        ), "total balance change is not balanced"
