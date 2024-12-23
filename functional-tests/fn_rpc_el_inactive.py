import flexitest
from web3 import Web3

import testenv
from utils import wait_until


@flexitest.register
class SeqStatusElInactiveTest(testenv.StrataTester):
    """
    Test that checks the behavior of client RPC when reth is down and ability to produce blocks
    when reth is up again
    """

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        reth = ctx.get_service("reth")
        # create sequencer RPC and wait until it is active
        seqrpc = seq.create_rpc()

        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )

        # wait for reth to be connected
        web3: Web3 = reth.create_web3()
        wait_until(lambda: web3.is_connected(), error_with="Reth did not start properly")

        # send 3 transaction so that reth can start after being stopped
        to_transfer = 1_000_000
        dest = web3.to_checksum_address("0x0000000000000000000000000006000000000001")
        transfer_balance(web3, dest, to_transfer)
        transfer_balance(web3, dest, to_transfer)
        transfer_balance(web3, dest, to_transfer)

        wait_until(
            lambda: web3.eth.get_balance(dest) == to_transfer * 3,
            error_with="Balance transfer not successful",
            timeout=10,
        )
        reth.stop()

        assert not web3.is_connected(), "Reth did not stop"

        # check if rpc is still working after sequencer has stopped
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer stopped after Reth stopped",
        )

        # check if sync status is working properly
        wait_until(
            lambda: seqrpc.strata_syncStatus() is not None,
            error_with="Sequencer stopped after Reth stopped",
        )

        # check if new l1 blocks are being recognized
        cur_l1_height = seqrpc.strata_l1status()["cur_height"]
        wait_until(
            lambda: seqrpc.strata_l1status()["cur_height"] > cur_l1_height,
            error_with="Sequencer stopped after Reth stopped",
        )

        reth.start()
        wait_until(lambda: web3.is_connected(), error_with="Reth did not start properly")

        # check if new blocks are being created again
        cur_slot = seqrpc.strata_clientStatus()["chain_tip_slot"]
        wait_until(
            lambda: seqrpc.strata_clientStatus()["chain_tip_slot"] > cur_slot,
            error_with="New blocks are not being created",
        )


def transfer_balance(web3: Web3, dest, to_transfer: int):
    source = web3.address
    web3.eth.send_transaction(
        {"to": dest, "value": hex(to_transfer), "gas": hex(100000), "from": source}
    )
