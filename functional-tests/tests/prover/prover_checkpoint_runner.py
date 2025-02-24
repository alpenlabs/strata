import time

import flexitest

from envs import testenv
from envs.testenv import BasicEnvConfig
from utils import (
    ProverClientSettings,
    RollupParamsSettings,
    wait_until,
)

# Test configuration for checkpoint-based prover
PROVER_CHECKPOINT_SETTINGS = {
    "CONSECUTIVE_PROOFS_REQUIRED": 3,
    "PROVER_TIMEOUT_SECONDS": 300,
}


@flexitest.register
class ProverCheckpointRunnerTest(testenv.StrataTester):
    """This tests the prover's checkpoint runner with an unreliable sequencer service."""

    def __init__(self, ctx: flexitest.InitContext):
        # Increase the proof timeout so that the checkpoint index increments only
        # after the prover client submits the corresponding checkpoint proof
        rollup_settings = RollupParamsSettings.new_default()
        rollup_settings.proof_timeout = PROVER_CHECKPOINT_SETTINGS["PROVER_TIMEOUT_SECONDS"]

        ctx.set_env(
            BasicEnvConfig(
                pre_generate_blocks=101,
                prover_client_settings=ProverClientSettings.new_with_proving(),
                rollup_settings=rollup_settings,
            )
        )

    def main(self, ctx: flexitest.RunContext):
        sequencer = ctx.get_service("sequencer")
        prover_client = ctx.get_service("prover_client")

        prover_rpc = prover_client.create_rpc()
        sequencer_rpc = sequencer.create_rpc()

        # Wait until the prover client reports readiness
        wait_until(
            lambda: prover_rpc.dev_strata_getReport() is not None,
            error_with="Prover did not start on time",
        )

        # Wait until the required number of consecutive checkpoint proofs are generated and verified
        wait_until(
            lambda: (
                sequencer_rpc.strata_getLatestCheckpointIndex()
                == PROVER_CHECKPOINT_SETTINGS["CONSECUTIVE_PROOFS_REQUIRED"]
            ),
            timeout=PROVER_CHECKPOINT_SETTINGS["PROVER_TIMEOUT_SECONDS"],
        )

        sequencer.stop()
        # Wait some time to make sure checkpoint runner indeed polls
        # on the checkpoint with the stopped (unreliable) sequencer.
        time.sleep(10)
        sequencer.start()
        sequencer_rpc = sequencer.create_rpc()

        # Wait until another portion of consecutive proofs are generated and verified
        # after the restart of the sequencer.
        wait_until(
            lambda: (
                sequencer_rpc.strata_getLatestCheckpointIndex()
                == PROVER_CHECKPOINT_SETTINGS["CONSECUTIVE_PROOFS_REQUIRED"] * 2
            ),
            timeout=PROVER_CHECKPOINT_SETTINGS["PROVER_TIMEOUT_SECONDS"],
        )
