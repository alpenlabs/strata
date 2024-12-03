#!/usr/bin/env python3

import os
import sys
import time
from math import ceil
from typing import Optional

import flexitest
from strata_utils import get_address, get_recovery_address

import factory
import net_settings
from constants import *
from rollup_params_cfg import RollupConfig
from utils import *


class BasicLiveEnv(flexitest.LiveEnv):
    """
    A common thin layer for all instances of the Environments.
    """

    def __init__(self, srvs, path, bridge_pk):
        super().__init__(srvs)
        self._el_address_gen = (
            f"deada00{x:04X}dca3ebeefdeadf001900dca3ebeef" for x in range(16**4)
        )
        self._ext_btc_addr_idx = 0
        self._rec_btc_addr_idx = 0
        self._bridge_pk = bridge_pk
        self.path = path

    def gen_el_address(self) -> str:
        """
        Generates a unique EL address to be used across tests.
        """
        return next(self._el_address_gen)

    def gen_ext_btc_address(self) -> str | List[str]:
        """
        Generates a unique bitcoin (external) taproot addresses that is funded with some BTC.
        """

        tr_addr: str = get_address(self._ext_btc_addr_idx)
        self._ext_btc_addr_idx += 1
        return tr_addr

    def gen_rec_btc_address(self) -> str | List[str]:
        """
        Generates a unique bitcoin (recovery) taproot addresses that is funded with some BTC.
        """

        rec_tr_addr: str = get_recovery_address(self._rec_btc_addr_idx, self._bridge_pk)
        self._rec_btc_addr_idx += 1
        return rec_tr_addr


class BasicEnvConfig(flexitest.EnvConfig):
    def __init__(
        self,
        pre_generate_blocks: int = 0,
        rollup_settings: Optional[RollupParamsSettings] = None,
        auto_generate_blocks: bool = True,
        enable_prover_client: bool = False,
        pre_fund_addrs: bool = True,
        n_operators: int = 2,
        message_interval: int = DEFAULT_ROLLUP_PARAMS["message_interval"],
        custom_chain: str = "dev",
    ):
        super().__init__()
        self.pre_generate_blocks = pre_generate_blocks
        self.rollup_settings = rollup_settings
        self.auto_generate_blocks = auto_generate_blocks
        self.enable_prover_client = enable_prover_client
        self.pre_fund_addrs = pre_fund_addrs
        self.n_operators = n_operators
        self.message_interval = message_interval
        self.custom_chain = custom_chain

    def init(self, ctx: flexitest.EnvContext) -> flexitest.LiveEnv:
        btc_fac = ctx.get_factory("bitcoin")
        seq_fac = ctx.get_factory("sequencer")
        reth_fac = ctx.get_factory("reth")
        bridge_fac = ctx.get_factory("bridge_client")

        svcs = {}

        # set up network params
        initdir = ctx.make_service_dir("_init")
        settings = self.rollup_settings or RollupParamsSettings.new_default()
        params_gen_data = generate_simple_params(initdir, settings, self.n_operators)
        params = params_gen_data["params"]
        # Instantiaze the generated rollup config so it's convenient to work with.
        rollup_cfg = RollupConfig.model_validate_json(params)

        # Construct the bridge pubkey from the config.
        # Technically, we could use utils::get_bridge_pubkey, but this makes sequencer
        # a dependency of pre-funding logic and just complicates the env setup.
        bridge_pk = get_bridge_pubkey_from_cfg(rollup_cfg)
        # TODO also grab operator keys and launch operators

        # reth needs some time to startup, start it first
        secret_dir = ctx.make_service_dir("secret")
        reth_secret_path = os.path.join(secret_dir, "jwt.hex")

        with open(reth_secret_path, "w") as f:
            f.write(generate_jwt_secret())

        reth = reth_fac.create_exec_client(
            0, reth_secret_path, None, custom_chain=self.custom_chain
        )
        reth_port = reth.get_prop("rpc_port")

        bitcoind = btc_fac.create_regtest_bitcoin()
        svcs["bitcoin"] = bitcoind
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS)

        brpc = bitcoind.create_rpc()
        walletname = bitcoind.get_prop("walletname")
        brpc.proxy.createwallet(walletname)
        seqaddr = brpc.proxy.getnewaddress()

        if self.pre_generate_blocks > 0:
            if self.pre_fund_addrs:
                # Since the pre-funding is enabled, we have to ensure the amount of pre-generated
                # blocks is enough to deal with the coinbase maturation.
                # Also, leave a log-message to indicate that the setup is little inconsistent.
                if self.pre_generate_blocks < 101:
                    print(
                        "Env setup: pre_fund_addrs is enabled, specify pre_generate_blocks >= 101."
                    )
                    self.pre_generate_blocks = 101

            chunk_size = 500
            while self.pre_generate_blocks > 0:
                batch_size = min(self.pre_generate_blocks, 1000)

                # generate blocks in chunks to avoid timeout
                num_chunks = ceil(batch_size / chunk_size)
                for i in range(0, batch_size, chunk_size):
                    chunk = int(i / chunk_size) + 1
                    num_blocks = int(min(chunk_size, batch_size - (chunk - 1) * chunk_size))
                    chunk = f"{chunk}/{num_chunks}"

                    print(
                        f"Pre generating {num_blocks} blocks to address {seqaddr}; chunk = {chunk}"
                    )
                    brpc.proxy.generatetoaddress(chunk_size, seqaddr)

                self.pre_generate_blocks -= batch_size

            if self.pre_fund_addrs:
                # Send funds for btc external and recovery addresses used in the test logic.
                # Generate one more block so the transaction is on the blockchain.
                brpc.proxy.sendmany(
                    "",
                    {
                        get_recovery_address(i, bridge_pk) if i < 100 else get_address(i - 100): 50
                        for i in range(200)
                    },
                )
                brpc.proxy.generatetoaddress(1, seqaddr)

        # generate blocks every 500 millis
        if self.auto_generate_blocks:
            generate_blocks(brpc, BLOCK_GENERATION_INTERVAL_SECS, seqaddr)

        rpc_port = bitcoind.get_prop("rpc_port")
        rpc_user = bitcoind.get_prop("rpc_user")
        rpc_pass = bitcoind.get_prop("rpc_password")
        rpc_sock = f"localhost:{rpc_port}/wallet/{walletname}"
        bitcoind_config = {
            "bitcoind_sock": rpc_sock,
            "bitcoind_user": rpc_user,
            "bitcoind_pass": rpc_pass,
        }
        reth_config = {
            "reth_socket": f"localhost:{reth_port}",
            "reth_secret_path": reth_secret_path,
        }
        sequencer = seq_fac.create_sequencer(bitcoind_config, reth_config, seqaddr, params)

        # Need to wait for at least `genesis_l1_height` blocks to be generated.
        # Sleeping some more for safety
        if self.auto_generate_blocks:
            time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 10)

        svcs["sequencer"] = sequencer
        svcs["reth"] = reth

        # Create all the bridge clients.
        for i in range(self.n_operators):
            xpriv_path = params_gen_data["opseedpaths"][i]
            xpriv = None
            with open(xpriv_path) as f:
                xpriv = f.read().strip()
            seq_url = sequencer.get_prop("rpc_url")
            br = bridge_fac.create_operator(
                xpriv, seq_url, bitcoind_config, message_interval=self.message_interval
            )
            name = f"bridge.{i}"
            svcs[name] = br

        if self.enable_prover_client:
            seq_port = sequencer.get_prop("rpc_port")
            reth_rpc_http_port = reth.get_prop("eth_rpc_http_port")

            prover_client_fac = ctx.get_factory("prover_client")
            prover_client = prover_client_fac.create_prover_client(
                bitcoind_config,
                f"http://localhost:{seq_port}",
                f"http://localhost:{reth_rpc_http_port}",
            )
            svcs["prover_client"] = prover_client

        return BasicLiveEnv(svcs, ctx.envdd_path, bridge_pk)


class HubNetworkEnvConfig(flexitest.EnvConfig):
    def __init__(
        self,
        pre_generate_blocks: int = 0,
        rollup_settings: Optional[RollupParamsSettings] = None,
        auto_generate_blocks: bool = True,
        n_operators: int = 2,
    ):
        self.pre_generate_blocks = pre_generate_blocks
        self.rollup_settings = rollup_settings
        self.auto_generate_blocks = auto_generate_blocks
        self.n_operators = n_operators
        super().__init__()

    def init(self, ctx: flexitest.EnvContext) -> flexitest.LiveEnv:
        btc_fac = ctx.get_factory("bitcoin")
        seq_fac = ctx.get_factory("sequencer")
        reth_fac = ctx.get_factory("reth")
        fn_fac = ctx.get_factory("fullnode")
        bridge_fac = ctx.get_factory("bridge_client")

        # set up network params
        initdir = ctx.make_service_dir("_init")
        settings = self.rollup_settings or RollupParamsSettings.new_default()
        params_gen_data = generate_simple_params(initdir, settings, self.n_operators)
        params = params_gen_data["params"]
        # Instantiaze the generated rollup config so it's convenient to work with.
        rollup_cfg = RollupConfig.model_validate_json(params)

        # Construct the bridge pubkey from the config.
        # Technically, we could use utils::get_bridge_pubkey, but this makes sequencer
        # a dependency of pre-funding logic and just complicates the env setup.
        bridge_pk = get_bridge_pubkey_from_cfg(rollup_cfg)
        # TODO also grab operator keys and launch operators

        # reth needs some time to startup, start it first
        secret_dir = ctx.make_service_dir("secret")
        reth_secret_path = os.path.join(secret_dir, "jwt.hex")

        with open(reth_secret_path, "w") as file:
            file.write(generate_jwt_secret())

        reth = reth_fac.create_exec_client(0, reth_secret_path, None)
        seq_reth_rpc_port = reth.get_prop("eth_rpc_http_port")
        fullnode_reth = reth_fac.create_exec_client(
            1, reth_secret_path, f"http://localhost:{seq_reth_rpc_port}"
        )
        reth_authrpc_port = reth.get_prop("rpc_port")

        bitcoind = btc_fac.create_regtest_bitcoin()
        # wait for services to to startup
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS)

        brpc = bitcoind.create_rpc()

        walletname = "dummy"
        brpc.proxy.createwallet(walletname)

        seqaddr = brpc.proxy.getnewaddress()

        if self.pre_generate_blocks > 0:
            print(f"Pre generating {self.pre_generate_blocks} blocks to address {seqaddr}")
            brpc.proxy.generatetoaddress(self.pre_generate_blocks, seqaddr)

        # generate blocks every 500 millis
        if self.auto_generate_blocks:
            generate_blocks(brpc, BLOCK_GENERATION_INTERVAL_SECS, seqaddr)
        rpc_port = bitcoind.get_prop("rpc_port")
        rpc_user = bitcoind.get_prop("rpc_user")
        rpc_pass = bitcoind.get_prop("rpc_password")
        rpc_sock = f"localhost:{rpc_port}/wallet/{walletname}"
        bitcoind_config = {
            "bitcoind_sock": rpc_sock,
            "bitcoind_user": rpc_user,
            "bitcoind_pass": rpc_pass,
        }
        reth_config = {
            "reth_socket": f"localhost:{reth_authrpc_port}",
            "reth_secret_path": reth_secret_path,
        }
        sequencer = seq_fac.create_sequencer(bitcoind_config, reth_config, seqaddr, params)
        # Need to wait for at least `genesis_l1_height` blocks to be generated.
        # Sleeping some more for safety
        if self.auto_generate_blocks:
            time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 10)

        fullnode_reth_port = fullnode_reth.get_prop("rpc_port")
        fullnode_reth_config = {
            "reth_socket": f"localhost:{fullnode_reth_port}",
            "reth_secret_path": reth_secret_path,
        }

        sequencer_rpc = f"ws://localhost:{sequencer.get_prop('rpc_port')}"

        fullnode = fn_fac.create_fullnode(
            bitcoind_config,
            fullnode_reth_config,
            sequencer_rpc,
            params,
        )

        svcs = {
            "bitcoin": bitcoind,
            "seq_node": sequencer,
            "seq_reth": reth,
            "follower_1_node": fullnode,
            "follower_1_reth": fullnode_reth,
        }

        # Create all the bridge clients.
        for i in range(self.n_operators):
            xpriv_path = params_gen_data["opseedpaths"][i]
            xpriv = None
            with open(xpriv_path) as f:
                xpriv = f.read().strip()
            seq_url = sequencer.get_prop("rpc_url")
            br = bridge_fac.create_operator(xpriv, seq_url, bitcoind_config)
            name = f"bridge.{i}"
            svcs[name] = br

        return BasicLiveEnv(svcs, bridge_pk)


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

    datadir_root = flexitest.create_datadir_in_workspace(os.path.join(test_dir, DD_ROOT))
    log_level = setup_root_logger()
    setup_test_loggers(datadir_root, tests, log_level)
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
        "basic": BasicEnvConfig(101),
        # Operator lag is a test that checks if the bridge can handle operator lag.
        # It is also useful for testing the reclaim path.
        "operator_lag": BasicEnvConfig(101, message_interval=10 * 60 * 1_000),
        # Devnet production env
        "devnet": BasicEnvConfig(101, custom_chain="devnet"),
        "fast_batches": BasicEnvConfig(101, rollup_settings=net_settings.get_fast_batch_settings()),
        "hub1": HubNetworkEnvConfig(
            2
        ),  # TODO: Need to generate at least horizon blocks, based on params
        "prover": BasicEnvConfig(101, enable_prover_client=True),
    }

    rt = flexitest.TestRuntime(global_envs, datadir_root, factories)
    rt.prepare_registered_tests()

    results = rt.run_tests(tests)
    rt.save_json_file("results.json", results)
    flexitest.dump_results(results)

    flexitest.fail_on_error(results)

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
