import time
from typing import Optional

import flexitest
import web3.eth
from web3 import Web3

from envs import testenv
from utils.reth import get_chainconfig

EPOCH_GAS_LIMIT = 2_000_000

chain_config = get_chainconfig()
chain_config["gasLimit"] = hex(1_000_000)


@flexitest.register
class ElBalanceTransferTest(testenv.StrataTester):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(
            testenv.BasicEnvConfig(110, epoch_gas_limit=EPOCH_GAS_LIMIT, custom_chain=chain_config)
        )

    def main(self, ctx: flexitest.RunContext):
        seq_signer = ctx.get_service("sequencer_signer")
        seq_signer.stop()
        # FIXME: process is NOT terminated yet so need to wait
        time.sleep(1)

        # seq = ctx.get_service("sequencer")
        # seqrpc = seq.create_rpc()
        reth = ctx.get_service("reth")
        web3: Web3 = reth.create_web3()

        source = web3.address
        nonce = web3.eth.get_transaction_count(source)
        _txids = [
            make_gas_burner_transaction(web3, source, nonce + i, 450_000) for i in range(0, 10)
        ]

        original_block_no = web3.eth.get_block_number()
        seq_signer.start()

        total_gas_used = 0
        block_no = original_block_no + 1
        zero_gas_blocks = 0
        while zero_gas_blocks < 2:
            while web3.eth.get_block_number() < block_no:
                self.info("no block yet")
                time.sleep(1)

            header = web3.eth.get_block(block_no)
            self.info(f"block_number: {header['number']}, gas_used: {header['gasUsed']}")

            if header["gasUsed"] == 0:
                zero_gas_blocks += 1
            else:
                zero_gas_blocks = 0

            total_gas_used += header["gasUsed"]
            block_no += 1

        self.info(f"total gas used: {total_gas_used}")

        assert total_gas_used <= EPOCH_GAS_LIMIT, "epoch gas should be limited"


def make_gas_burner_transaction(
    web3: web3.Web3, address: str, nonce: int, burn_gas: int, gas_limit: Optional[int] = None
):
    """
    Performs a token transfer transaction to own account with a large calldata.
    Sends enough calldata to consume `burn_gas` gas.
    Note: reth has default calldata limit of 128kb = ~ 2M gas

    :param web3: Web3 instance.
    :param burn_gas: Amount of gas to burn through calldata.
    :param gas_limit: Custom gas limit to use.
    :return: Transaction id
    """
    # each non-zero byte calldata consumes 16 gas
    data = "0x" + "01" * (burn_gas // 16)

    tx_params = {
        "to": address,
        "value": 0,
        "gas": gas_limit or burn_gas + 21000,
        "data": data,
        "from": address,
        "nonce": hex(nonce),
    }
    txid = web3.eth.send_transaction(tx_params)
    print("txid", txid.to_0x_hex())
    return txid
