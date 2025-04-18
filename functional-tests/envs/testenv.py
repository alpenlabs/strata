import json
import time
from typing import Optional

import flexitest
from strata_utils import (
    get_address,
    get_recovery_address,
)

from envs.rollup_params_cfg import RollupConfig
from factory.config import BitcoindConfig, RethELConfig
from load.cfg import LoadConfig, LoadConfigBuilder
from utils import *
from utils.constants import *


class StrataTester(flexitest.Test):
    """
    Class to be used instead of flexitest.Test for accessing logger
    """

    def premain(self, ctx: flexitest.RunContext):
        logger = setup_test_logger(ctx.datadir_root, ctx.name)
        self.debug = logger.debug
        self.info = logger.info
        self.warning = logger.warning
        self.error = logger.error
        self.critical = logger.critical


class StrataTestRuntime(flexitest.TestRuntime):
    """
    Extended testenv.StrataTestRuntime to call custom run context
    """

    def create_run_context(self, name: str, env: flexitest.LiveEnv) -> flexitest.RunContext:
        return StrataRunContext(self.datadir_root, name, env)


class StrataRunContext(flexitest.RunContext):
    """
    Custom run context which provides access to services and some test specific variables.
    To be used by ExtendedTestRuntime
    """

    def __init__(self, datadir_root: str, name: str, env: flexitest.LiveEnv):
        self.name = name
        self.datadir_root = datadir_root
        super().__init__(env)


class BasicLiveEnv(flexitest.LiveEnv):
    """
    A common thin layer for all instances of the Environments.
    """

    def __init__(self, srvs, bridge_pk, rollup_cfg: RollupConfig):
        super().__init__(srvs)
        self._el_address_gen = (
            f"deada00{x:04X}dca3ebeefdeadf001900dca3ebeef" for x in range(16**4)
        )
        self._ext_btc_addr_idx = 0
        self._rec_btc_addr_idx = 0
        self._bridge_pk = bridge_pk
        self._rollup_cfg = rollup_cfg

    def gen_el_address(self) -> str:
        """
        Generates a unique EL address to be used across tests.
        """
        return next(self._el_address_gen)

    def gen_ext_btc_address(self) -> str | list[str]:
        """
        Generates a unique bitcoin (external) taproot addresses that is funded with some BTC.
        """

        tr_addr: str = get_address(self._ext_btc_addr_idx)
        self._ext_btc_addr_idx += 1
        return tr_addr

    def gen_rec_btc_address(self) -> str | list[str]:
        """
        Generates a unique bitcoin (recovery) taproot addresses that is funded with some BTC.
        """

        rec_tr_addr: str = get_recovery_address(self._rec_btc_addr_idx, self._bridge_pk)
        self._rec_btc_addr_idx += 1
        return rec_tr_addr

    def rollup_cfg(self) -> RollupConfig:
        return self._rollup_cfg


class BasicEnvConfig(flexitest.EnvConfig):
    def __init__(
        self,
        pre_generate_blocks: int = 0,
        rollup_settings: Optional[RollupParamsSettings] = None,
        prover_client_settings: Optional[ProverClientSettings] = None,
        auto_generate_blocks: bool = True,
        pre_fund_addrs: bool = True,
        n_operators: int = 2,
        message_interval: int = 0,
        duty_timeout_duration: int = 10,
        custom_chain: str | dict = "dev",
        epoch_gas_limit: Optional[int] = None,
    ):
        super().__init__()
        self.pre_generate_blocks = pre_generate_blocks
        self.rollup_settings = rollup_settings
        self.prover_client_settings = prover_client_settings
        self.auto_generate_blocks = auto_generate_blocks
        self.pre_fund_addrs = pre_fund_addrs
        self.n_operators = n_operators
        self.message_interval = message_interval
        self.duty_timeout_duration = duty_timeout_duration
        self.custom_chain = custom_chain
        self.epoch_gas_limit = epoch_gas_limit

    def init(self, ctx: flexitest.EnvContext) -> flexitest.LiveEnv:
        btc_fac = ctx.get_factory("bitcoin")
        seq_fac = ctx.get_factory("sequencer")
        seq_signer_fac = ctx.get_factory("sequencer_signer")
        reth_fac = ctx.get_factory("reth")

        svcs = {}

        # set up network params
        initdir = ctx.make_service_dir("_init")

        custom_chain = self.custom_chain
        if isinstance(custom_chain, dict):
            json_path = os.path.join(initdir, "custom_chain.json")
            with open(json_path, "w") as f:
                json.dump(custom_chain, f)
            custom_chain = json_path

        settings = (
            self.rollup_settings or RollupParamsSettings.new_default().fast_batch().strict_mode()
        )
        if custom_chain != self.custom_chain:
            settings = settings.with_chainconfig(custom_chain)
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

        reth = reth_fac.create_exec_client(0, reth_secret_path, None, custom_chain=custom_chain)
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
                if self.pre_generate_blocks < 110:
                    print(
                        "Env setup: pre_fund_addrs is enabled, specify pre_generate_blocks >= 110."
                    )
                    self.pre_generate_blocks = 110

            while self.pre_generate_blocks > 0:
                batch_size = min(self.pre_generate_blocks, 500)

                print(f"Pre generating {batch_size} blocks to address {seqaddr}")
                brpc.proxy.generatetoaddress(batch_size, seqaddr)
                self.pre_generate_blocks -= batch_size

            if self.pre_fund_addrs:
                # Send funds for btc external and recovery addresses used in the test logic.
                # Generate one more block so the transaction is on the blockchain.
                brpc.proxy.sendmany(
                    "",
                    {
                        get_recovery_address(i, bridge_pk) if i < 10 else get_address(i - 10): 20
                        for i in range(20)
                    },
                )
                brpc.proxy.generatetoaddress(1, seqaddr)

        # generate blocks every 500 millis
        if self.auto_generate_blocks:
            generate_blocks(brpc, BLOCK_GENERATION_INTERVAL_SECS, seqaddr)

        rpc_port = bitcoind.get_prop("rpc_port")
        rpc_sock = f"localhost:{rpc_port}/wallet/{walletname}"
        bitcoind_config = BitcoindConfig(
            rpc_url=rpc_sock,
            rpc_user=bitcoind.get_prop("rpc_user"),
            rpc_password=bitcoind.get_prop("rpc_password"),
        )

        reth_config = RethELConfig(
            rpc_url=f"localhost:{reth_port}",
            secret=reth_secret_path,
        )
        reth_rpc_http_port = reth.get_prop("eth_rpc_http_port")

        sequencer = seq_fac.create_sequencer_node(bitcoind_config, reth_config, seqaddr, params)

        seq_host = sequencer.get_prop("rpc_host")
        seq_port = sequencer.get_prop("rpc_port")
        sequencer_signer = seq_signer_fac.create_sequencer_signer(
            seq_host, seq_port, epoch_gas_limit=self.epoch_gas_limit
        )

        svcs["sequencer"] = sequencer
        svcs["sequencer_signer"] = sequencer_signer
        svcs["reth"] = reth

        # Need to wait for at least `genesis_l1_height` blocks to be generated.
        # Sleeping some more for safety
        if self.auto_generate_blocks:
            time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 10)

        prover_client_fac = ctx.get_factory("prover_client")
        prover_client_settings = self.prover_client_settings or ProverClientSettings.new_default()
        prover_client = prover_client_fac.create_prover_client(
            bitcoind_config,
            f"http://localhost:{seq_port}",
            f"http://localhost:{reth_rpc_http_port}",
            params,
            prover_client_settings,
        )
        svcs["prover_client"] = prover_client

        return BasicLiveEnv(svcs, bridge_pk, rollup_cfg)


class HubNetworkEnvConfig(flexitest.EnvConfig):
    def __init__(
        self,
        pre_generate_blocks: int = 0,
        rollup_settings: Optional[RollupParamsSettings] = None,
        auto_generate_blocks: bool = True,
        n_operators: int = 2,
        duty_timeout_duration: int = 10,
    ):
        self.pre_generate_blocks = pre_generate_blocks
        self.rollup_settings = rollup_settings
        self.auto_generate_blocks = auto_generate_blocks
        self.n_operators = n_operators
        self.duty_timeout_duration = duty_timeout_duration
        super().__init__()

    def init(self, ctx: flexitest.EnvContext) -> flexitest.LiveEnv:
        btc_fac = ctx.get_factory("bitcoin")
        seq_fac = ctx.get_factory("sequencer")
        seq_signer_fac = ctx.get_factory("sequencer_signer")
        reth_fac = ctx.get_factory("reth")
        fn_fac = ctx.get_factory("fullnode")

        # set up network params
        initdir = ctx.make_service_dir("_init")
        settings = self.rollup_settings or RollupParamsSettings.new_default().fast_batch()
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
        rpc_sock = f"localhost:{rpc_port}/wallet/{walletname}"
        bitcoind_config = BitcoindConfig(
            rpc_url=rpc_sock,
            rpc_user=bitcoind.get_prop("rpc_user"),
            rpc_password=bitcoind.get_prop("rpc_password"),
        )

        reth_config = RethELConfig(
            rpc_url=f"localhost:{reth_authrpc_port}",
            secret=reth_secret_path,
        )
        reth_rpc_http_port = reth.get_prop("eth_rpc_http_port")
        sequencer = seq_fac.create_sequencer_node(bitcoind_config, reth_config, seqaddr, params)

        seq_host = sequencer.get_prop("rpc_host")
        seq_port = sequencer.get_prop("rpc_port")
        sequencer_signer = seq_signer_fac.create_sequencer_signer(seq_host, seq_port)

        # Need to wait for at least `genesis_l1_height` blocks to be generated.
        # Sleeping some more for safety
        if self.auto_generate_blocks:
            time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 10)

        fullnode_reth_port = fullnode_reth.get_prop("rpc_port")
        fullnode_reth_config = RethELConfig(
            rpc_url=f"localhost:{fullnode_reth_port}",
            secret=reth_secret_path,
        )

        sequencer_rpc = f"ws://localhost:{sequencer.get_prop('rpc_port')}"

        fullnode = fn_fac.create_fullnode(
            bitcoind_config,
            fullnode_reth_config,
            sequencer_rpc,
            params,
        )

        prover_client_fac = ctx.get_factory("prover_client")
        prover_client_settings = ProverClientSettings.new_with_proving()
        prover_client = prover_client_fac.create_prover_client(
            bitcoind_config,
            f"http://localhost:{seq_port}",
            f"http://localhost:{reth_rpc_http_port}",
            params,
            prover_client_settings,
        )

        svcs = {
            "bitcoin": bitcoind,
            "seq_node": sequencer,
            "sequencer_signer": sequencer_signer,
            "seq_reth": reth,
            "follower_1_node": fullnode,
            "follower_1_reth": fullnode_reth,
            "prover_client": prover_client,
        }

        return BasicLiveEnv(svcs, bridge_pk, rollup_cfg)


# TODO: Maybe, we need to make it dynamic to enhance any EnvConfig with load testing capabilities.
class LoadEnvConfig(BasicEnvConfig):
    _load_cfgs: list[LoadConfigBuilder] = []

    def with_load_builder(self, builder: LoadConfigBuilder):
        self._load_cfgs.append(builder)
        return self

    def init(self, ctx: flexitest.EnvContext) -> flexitest.LiveEnv:
        basic_live_env = super().init(ctx)

        if not self._load_cfgs:
            raise Exception(
                "LoadEnv has no load builders! Specify load builders or just use BasicEnv."
            )

        # Create load generator services for all the builders.
        svcs = basic_live_env.svcs
        load_fac = ctx.get_factory("load_generator")
        for builder in self._load_cfgs:
            load_cfg: LoadConfig = builder(svcs)
            svcs[f"load_generator.{builder.name}"] = load_fac.create_simple_loadgen(load_cfg)

        return BasicLiveEnv(svcs, basic_live_env._bridge_pk, basic_live_env._rollup_cfg)
