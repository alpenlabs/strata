import time

import flexitest
from web3 import Web3


@flexitest.register
class ElBridgePrecompileTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        reth = ctx.get_service("reth")
        web3: Web3 = reth.create_web3()

        source = web3.address
        dest = web3.to_checksum_address("0x000000000000000000000000000000000b121d9e")
        # 64 bytes
        data = "0x" + "00" * 64

        assert web3.is_connected(), "cannot connect to reth"

        original_block_no = web3.eth.block_number
        original_balance = web3.eth.get_balance(dest)

        print(original_block_no, original_balance)

        # 1 eth
        to_transfer = 1_000_000_000_000_000_000

        txid = web3.eth.send_transaction(
            {
                "to": dest,
                "value": hex(to_transfer),
                "gas": hex(100000),
                "from": source,
                "data": data,
            }
        )
        print(txid.to_0x_hex())

        # build block
        time.sleep(2)

        receipt = web3.eth.get_transaction_receipt(txid)
        # print(receipt)

        assert receipt.status == 1, "precompile transaction failed"
        assert len(receipt.logs) == 1, "no logs or invalid logs"

        final_block_no = web3.eth.block_number
        final_balance = web3.eth.get_balance(dest)

        print(final_block_no, final_balance)

        assert original_block_no < final_block_no, "not building blocks"
        assert original_balance + to_transfer == final_balance, "balance not updated"
