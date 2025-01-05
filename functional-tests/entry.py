#!/usr/bin/env python3

import os
import sys

import flexitest

import factory
import net_settings
import testenv
from constants import *
from utils import *


def main(argv):
    test_dir = os.path.dirname(os.path.abspath(__file__))
    modules = flexitest.runtime.scan_dir_for_modules(test_dir)
    all_tests = flexitest.runtime.load_candidate_modules(modules)

    # Avoid running prover related tets while running all the tests
    # Filter the prover test files if not present in argv
    if len(argv) > 1:
        # Run the specific test file passed as the first argument (without .py extension)
        tests = [str(tst).removesuffix(".py") for tst in argv[1:]]
    else:
        # Run all tests, excluding those containing "fn_prover", unless explicitly passed in argv
        tests = [test for test in all_tests if "fn_prover" not in test or test in argv]

    btc_fac = factory.BitcoinFactory([12300 + i for i in range(30)])
    seq_fac = factory.StrataFactory([12400 + i for i in range(30)])
    fullnode_fac = factory.FullNodeFactory([12500 + i for i in range(30)])
    reth_fac = factory.RethFactory([12600 + i for i in range(20 * 3)])
    prover_client_fac = factory.ProverClientFactory([12700 + i for i in range(20 * 3)])
    bridge_client_fac = factory.BridgeClientFactory([12800 + i for i in range(30)])

    factories = {
        "bitcoin": btc_fac,
        "sequencer": seq_fac,
        "fullnode": fullnode_fac,
        "reth": reth_fac,
        "prover_client": prover_client_fac,
        "bridge_client": bridge_client_fac,
    }

    global_envs = {
        # Basic env is the default env for all tests.
        "basic": testenv.BasicEnvConfig(101),
        # Operator lag is a test that checks if the bridge can handle operator lag.
        # It is also useful for testing the reclaim path.
        "operator_lag": testenv.BasicEnvConfig(101, message_interval=10 * 60 * 1_000),
        # Devnet production env
        "devnet": testenv.BasicEnvConfig(101, custom_chain="devnet"),
        "fast_batches": testenv.BasicEnvConfig(
            101, rollup_settings=net_settings.get_fast_batch_settings()
        ),
        "hub1": testenv.HubNetworkEnvConfig(
            2
        ),  # TODO: Need to generate at least horizon blocks, based on params
        "prover": testenv.BasicEnvConfig(101),
    }

    setup_root_logger()
    datadir_root = flexitest.create_datadir_in_workspace(os.path.join(test_dir, DD_ROOT))
    rt = testenv.StrataTestRuntime(global_envs, datadir_root, factories)
    rt.prepare_registered_tests()

    results = rt.run_tests(tests)
    rt.save_json_file("results.json", results)
    flexitest.dump_results(results)

    flexitest.fail_on_error(results)

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
