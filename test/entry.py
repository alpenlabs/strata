#!/usr/bin/env python3

import os
import sys

import flexitest

class BitcoinFactory(flexitest.Factory):
    def __init__(self, datadir_pfx: str, port_range: list[int]):
        super().__init__(datadir_pfx, port_range)

    def create_regtest_bitcoin(self) -> flexitest.Service:
        # TODO
        raise NotImplementedError()

class VertexFactory(flexitest.Factory):
    def __init__(self, datadir_pfx: str, port_range: list[int]):
        super().__init__(datadir_pfx, port_range)

    def create_sequencer(self, bitcoin_rpc: str) -> flexitest.Service:
        # TODO
        raise NotImplementedError()

class BasicEnvConfig(flexitest.EnvConfig):
    def __init__(self):
        pass

    def init(self, facs: dict) -> flexitest.LiveEnv:
        btc_fac = facs["bitcoin"]
        seq_fac = facs["sequencer"]

        bitcoind = btc_fac.create_regtest_bitcoin()
        bitcoin_rpc = "// TODO"
        sequencer = seq_fac.create_sequencer(bitcoin_rpc)

        svcs = {"bitcoin": bitcoind, "sequencer": sequencer}
        return flexitest.LiveEnv(svcs)

def main(argv):
    test_dir = os.path.dirname(os.path.abspath(__file__))

    datadir_root = flexitest.create_datadir_in_workspace(os.path.join(test_dir, "_dd"))

    modules = flexitest.runtime.scan_dir_for_modules(test_dir)
    tests = flexitest.runtime.load_candidate_modules(modules)

    btc_fac = BitcoinFactory(datadir_root, [12300 + i for i in range(20)])
    seq_fac = VertexFactory(datadir_root, [12400 + i for i in range(20)])
    factories = {"bitcoin": btc_fac, "sequencer": seq_fac}
    envs = {"basic": BasicEnvConfig()}
    rt = flexitest.TestRuntime(envs, datadir_root, factories)
    rt.prepare_registered_tests()

    results = rt.run_tests(tests)
    flexitest.dump_results(results)

    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv))
