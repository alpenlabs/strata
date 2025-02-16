import logging
import json

import flexitest

from envs import net_settings, testenv
from utils import *


@flexitest.register
class BlockFinalizationTest(testenv.StrataTester):
    """ """

    def __init__(self, ctx: flexitest.InitContext):
        premine_blocks = 101
        settings = net_settings.get_fast_batch_settings()
        settings.genesis_trigger = premine_blocks + 5
        settings.proof_timeout = 5

        ctx.set_env(testenv.BasicEnvConfig(premine_blocks, rollup_settings=settings))

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")
        seqrpc = seq.create_rpc()

        prover = ctx.get_service("prover_client")
        prover_rpc = prover.create_rpc()

        num_epochs = 4

        epoch = wait_until_chain_epoch(seqrpc, num_epochs, timeout=30)
        logging.info(f"epoch summary: {epoch}")

        cstat = seqrpc.strata_clientStatus()
        cstatdump = json.dumps(cstat, indent=2)
        logging.info(f"client status: {cstatdump}")

        # Wait for prover
        # TODO What is this check for?
        wait_until(
            lambda: prover_rpc.dev_strata_getReport() is not None,
            error_with="Prover did not start on time",
        )

        check_submit_proof_fails_for_nonexistent_batch(seqrpc, 100)

        # Wait until we get the checkpoint confirmed.
        wait_until_epoch_confirmed(seqrpc, 1, timeout=30)

        # Check for first 4 checkpoints
        for n in range(num_epochs):
            check_nth_checkpoint_finalized(n, seqrpc, prover_rpc)
            logging.info(f"Pass checkpoint finalization for checkpoint {n}")

        # Proof for checkpoint 0 is already sent above
        check_already_sent_proof(seqrpc, 0)
