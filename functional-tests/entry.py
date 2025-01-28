#!/usr/bin/env python3
from gevent import monkey

monkey.patch_all()

import argparse
import os
import sys

import flexitest


from envs import net_settings, testenv
from factory import factory
from utils import *
from utils.constants import *


TEST_DIR: str = "tests"

# Initialize the parser with arguments.
parser = argparse.ArgumentParser(prog="entry.py")
parser.add_argument("-g", "--groups", nargs="*", help="Define the test groups to execute")
parser.add_argument("-t", "--tests", nargs="*", help="Define individual tests to execute")


def filter_tests(parsed_args, modules):
    """
    Filters test modules against parsed args supplied from the command line.
    """

    arg_groups = frozenset(parsed_args.groups or [])
    # Extract filenames from the tests paths.
    arg_tests = frozenset(
        [os.path.split(t)[1].removesuffix(".py") for t in parsed_args.tests or []]
    )

    filtered = dict()
    for test, path in modules.items():
        # Drop the prefix of the path before TEST_DIR
        test_path_parts = os.path.normpath(path).split(os.path.sep)
        # idx should never be None because TEST_DIR should be in the path.
        idx = next((i for i, part in enumerate(test_path_parts) if part == TEST_DIR), None)
        test_path_parts = test_path_parts[idx + 1 :]
        # The "groups" the current test belongs to.
        test_groups = frozenset(test_path_parts[:-1])

        # Filtering logic:
        # if groups or tests were specified (non-empty) as args, then check for exclusion
        take = True
        if arg_groups and not (arg_groups & test_groups):
            take = False
        if arg_tests and test not in arg_tests:
            take = False

        if take:
            filtered[test] = path

    return filtered


def main(argv):
    """
    The main entrypoint for running functional tests.
    """

    parsed_args = parser.parse_args(argv[1:])

    root_dir = os.path.dirname(os.path.abspath(__file__))
    test_dir = os.path.join(root_dir, TEST_DIR)
    modules = filter_tests(parsed_args, flexitest.runtime.scan_dir_for_modules(test_dir))
    tests = flexitest.runtime.load_candidate_modules(modules)

    btc_fac = factory.BitcoinFactory([12300 + i for i in range(100)])
    seq_fac = factory.StrataFactory([12400 + i for i in range(100)])
    fullnode_fac = factory.FullNodeFactory([12500 + i for i in range(100)])
    reth_fac = factory.RethFactory([12600 + i for i in range(100 * 3)])
    prover_client_fac = factory.ProverClientFactory([12900 + i for i in range(100 * 3)])
    bridge_client_fac = factory.BridgeClientFactory([13200 + i for i in range(100)])
    load_gen_fac = factory.LoadGeneratorFactory([14000 + i for i in range(100)])
    seq_signer_fac = factory.StrataSequencerFactory()

    factories = {
        "bitcoin": btc_fac,
        "sequencer": seq_fac,
        "sequencer_signer": seq_signer_fac,
        "fullnode": fullnode_fac,
        "reth": reth_fac,
        "prover_client": prover_client_fac,
        "bridge_client": bridge_client_fac,
        "load_generator": load_gen_fac,
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
        "load": testenv.LoadEnvConfig(),
    }

    setup_root_logger()
    datadir_root = flexitest.create_datadir_in_workspace(os.path.join(root_dir, DD_ROOT))
    rt = testenv.StrataTestRuntime(global_envs, datadir_root, factories)
    rt.prepare_registered_tests()

    results = rt.run_tests(tests)
    rt.save_json_file("results.json", results)
    flexitest.dump_results(results)
    # TODO(load): dump load test stats into separate file.

    flexitest.fail_on_error(results)

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
