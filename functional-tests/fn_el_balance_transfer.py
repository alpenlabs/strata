import time

import flexitest
from web3 import Web3


@flexitest.register
class ElBalanceTransferTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")

        web3: Web3 = reth.create_web3()
        # rethrpc = reth.create_rpc()

        source = web3.address
        dest = "0x0000000000000000000000000000000000000001"

        print(web3.is_connected())
        original_block_no = web3.eth.block_number
        original_balance = web3.eth.get_balance(dest)

        print(original_block_no, original_balance)
        to_transfer = 1_000_000_000_000_000_000

        for _ in range(6):
            print(
                web3.eth.send_transaction(
                    {"to": dest, "value": hex(to_transfer), "gas": hex(100000), "from": source}
                )
            )

            time.sleep(1)

            final_block_no = web3.eth.block_number
            final_balance = web3.eth.get_balance(dest)

            print(final_block_no, final_balance)

            # assert original_block_no < final_block_no
            # assert original_balance + to_transfer == final_balance
