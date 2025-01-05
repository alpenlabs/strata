import time

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient
from flexitest.service import Service

import net_settings
import testenv
from rollup_params_cfg import RollupConfig
from utils import (
    ManualGenBlocksConfig,
    check_nth_checkpoint_finalized,
    check_submit_proof_fails_for_nonexistent_batch,
    submit_checkpoint,
    wait_until,
)


@flexitest.register
class BitcoinReorgChecksTest(testenv.StrataTester):
    """This tests finalization when there is reorg on L1"""

    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env(
            testenv.BasicEnvConfig(
                # TODO: Need to generate at least horizon height blocks, can't
                # get rollup params from here
                2,
                rollup_settings=net_settings.get_fast_batch_settings(),
                auto_generate_blocks=False,
            )
        )

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        btc = ctx.get_service("bitcoin")
        prover = ctx.get_service("prover")

        seqrpc = seq.create_rpc()
        btcrpc: BitcoindClient = btc.create_rpc()
        seq_addr = seq.get_prop("address")

        cfg: RollupConfig = ctx.env.rollup_cfg()
        finality_depth = cfg.l1_reorg_safe_depth

        # Wait for seq
        wait_until(
            lambda: seqrpc.strata_protocolVersion() is not None,
            error_with="Sequencer did not start on time",
        )

        # First generate blocks to seq address
        btcrpc.proxy.generatetoaddress(101, seq_addr)
        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        manual_gen = ManualGenBlocksConfig(btcrpc, finality_depth, seq_addr)

        # Sanity Check for first checkpoint
        idx = 0
        check_nth_checkpoint_finalized(idx, seqrpc, manual_gen)
        self.debug(f"Pass checkpoint finalization for checkpoint {idx}")

        # TODO remove this after adding a proper config file
        # We need to wait for the tx to be published to L1
        time.sleep(0.5)
        # Test reorg, without pruning anything, let mempool and wallet retain the txs
        check_nth_checkpoint_finalized_on_reorg(idx + 1, seq, btc, prover)


def check_nth_checkpoint_finalized_on_reorg(checkpt_idx: int, seq: Service, btc: Service, prover: Service):
    # Now submit another checkpoint proof and produce a couple of blocks(less than reorg depth)
    seqrpc = seq.create_rpc()
    btcrpc = btc.create_rpc()
    prover_rpc = prover.create_rpc()

    seq_addr = seq.get_prop("address")
    cfg: RollupConfig = ctx.env.rollup_cfg()
    finality_depth = cfg.l1_reorg_safe_depth
    manual_gen = ManualGenBlocksConfig(btcrpc, finality_depth, seq_addr)

    # gen some blocks
    btcrpc.proxy.generatetoaddress(3, seq_addr)

    submit_checkpoint(checkpt_idx, seqrpc, prover_rpc, manual_gen)
    published_txid = seqrpc.strata_l1status()["last_published_txid"]

    # wait until it gets confirmed
    btcrpc.proxy.generatetoaddress(1, seq_addr)

    txinfo = btcrpc.proxy.gettransaction(published_txid)
    assert txinfo["confirmations"] > 0, "Tx should have some confirmations"

    # Get block height corresponding to the tx
    txinfo = btcrpc.proxy.gettransaction(published_txid)
    blockheight = txinfo["blockheight"]
    blockhash = btcrpc.proxy.getblockhash(blockheight)

    # Now invalidate the block
    btcrpc.proxy.invalidateblock(blockhash)

    # Validate tx is not actually in the chain
    txinfo = btcrpc.proxy.gettransaction(published_txid)
    assert txinfo["confirmations"] == 0, "Tx should have 0 confirmations"

    # Wait until the tx is possibly republished to l1(Will be republished if
    # inputs changed after reorg, or else the tx will be the same.
    # NOTE: This would ideally be done using `wait_until` but due to some issues with tracking
    # `last_published_txid` in L1Status, need to do this sleep wait hack
    time.sleep(4)

    new_addr = btcrpc.proxy.getnewaddress()
    # Create a block so that the inscription is included
    btcrpc.proxy.generatetoaddress(1, new_addr)

    # Create enough blocks to finalize
    btcrpc.proxy.generatetoaddress(finality_depth + 1, new_addr)

    batch_info = seqrpc.strata_getCheckpointInfo(checkpt_idx)
    to_finalize_blkid = batch_info["l2_blockid"]

    # Check finalized
    wait_until(
        lambda: seqrpc.strata_syncStatus()["finalized_block_id"] == to_finalize_blkid,
        error_with="Block not finalized",
        timeout=10,
    )
