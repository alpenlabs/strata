#!/usr/bin/env python3
import json
import os
import sys
import time
from math import ceil
from typing import Optional, TypedDict

import flexitest
import web3
import web3.middleware
from bitcoinlib.services.bitcoind import BitcoindClient

import seqrpc
from constants import (
    BD_PASSWORD,
    BD_USERNAME,
    BLOCK_GENERATION_INTERVAL_SECS,
    DD_ROOT,
    DEFAULT_ROLLUP_PARAMS,
    FAST_BATCH_ROLLUP_PARAMS,
    SEQ_KEY,
)
from utils import generate_blocks, generate_jwt_secret


class BitcoinFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)

    @flexitest.with_ectx("ctx")
    def create_regtest_bitcoin(self, ctx: flexitest.EnvContext) -> flexitest.Service:
        datadir = ctx.make_service_dir("bitcoin")
        p2p_port = self.next_port()
        rpc_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        cmd = [
            "bitcoind",
            "-txindex",
            "-regtest",
            "-printtoconsole",
            "-fallbackfee=0.00001",
            f"-datadir={datadir}",
            f"-port={p2p_port}",
            f"-rpcport={rpc_port}",
            f"-rpcuser={BD_USERNAME}",
            f"-rpcpassword={BD_PASSWORD}",
        ]

        props = {
            "rpc_port": rpc_port,
            "rpc_user": BD_USERNAME,
            "rpc_password": BD_PASSWORD,
            "walletname": "testwallet",
        }

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()

        def _create_rpc():
            url = f"http://{BD_USERNAME}:{BD_PASSWORD}@localhost:{rpc_port}"
            return BitcoindClient(base_url=url, network="regtest")

        svc.create_rpc = _create_rpc

        return svc


class BitcoinRpcConfig(TypedDict):
    bitcoind_sock: str
    bitcoind_user: str
    bitcoind_pass: str


class RethConfig(TypedDict):
    reth_socket: str
    reth_secret_path: str


class StrataFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)

    @flexitest.with_ectx("ctx")
    def create_sequencer(
        self,
        bitcoind_config: BitcoinRpcConfig,
        reth_config: RethConfig,
        sequencer_address: str,
        custom_rollup_params: Optional[dict],
        ctx: flexitest.EnvContext,
    ) -> flexitest.Service:
        datadir = ctx.make_service_dir("sequencer")
        rpc_port = self.next_port()
        rpc_host = "localhost"
        logfile = os.path.join(datadir, "service.log")

        keyfile = os.path.join(datadir, "seqkey.bin")
        seq_key = SEQ_KEY.hex()
        with open(keyfile, "w") as f:
            f.write(seq_key)

        # fmt: off
        cmd = [
            "strata-client",
            "--datadir", datadir,
            "--rpc-host", rpc_host,
            "--rpc-port", str(rpc_port),
            "--bitcoind-host", bitcoind_config["bitcoind_sock"],
            "--bitcoind-user", bitcoind_config["bitcoind_user"],
            "--bitcoind-password", bitcoind_config["bitcoind_pass"],
            "--reth-authrpc", reth_config["reth_socket"],
            "--reth-jwtsecret", reth_config["reth_secret_path"],
            "--network", "regtest",
            "--sequencer-key", keyfile,
            "--sequencer-bitcoin-address", sequencer_address,
        ]
        # fmt: on

        rollup_params_file = os.path.join(datadir, "rollup_params.json")
        rollup_params = custom_rollup_params if custom_rollup_params else DEFAULT_ROLLUP_PARAMS

        with open(rollup_params_file, "w") as f:
            json.dump(rollup_params, f)

        cmd.extend(["--rollup-params", rollup_params_file])

        props = {
            "rpc_host": rpc_host,
            "rpc_port": rpc_port,
            "seqkey": seq_key,
            "address": sequencer_address,
        }
        rpc_url = f"ws://{rpc_host}:{rpc_port}"

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()

        def _create_rpc():
            return seqrpc.JsonrpcClient(rpc_url)

        svc.create_rpc = _create_rpc

        return svc


class FullNodeFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)
        self.fn_count = 0

    @flexitest.with_ectx("ctx")
    def create_fullnode(
        self,
        bitcoind_config: BitcoinRpcConfig,
        reth_config: RethConfig,
        sequencer_rpc: str,
        custom_rollup_params: Optional[dict],
        ctx: flexitest.EnvContext,
    ) -> flexitest.Service:
        self.fn_count += 1
        id = self.fn_count

        datadir = ctx.make_service_dir(f"fullnode.{id}")
        rpc_host = "localhost"
        rpc_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        # fmt: off
        cmd = [
            "strata-client",
            "--datadir", datadir,
            "--rpc-host", rpc_host,
            "--rpc-port", str(rpc_port),
            "--bitcoind-host", bitcoind_config["bitcoind_sock"],
            "--bitcoind-user", bitcoind_config["bitcoind_user"],
            "--bitcoind-password", bitcoind_config["bitcoind_pass"],
            "--reth-authrpc", reth_config["reth_socket"],
            "--reth-jwtsecret", reth_config["reth_secret_path"],
            "--network", "regtest",
            "--sequencer-rpc", sequencer_rpc,
        ]
        # fmt: on

        rollup_params_file = os.path.join(datadir, "rollup_params.json")
        rollup_params = custom_rollup_params if custom_rollup_params else DEFAULT_ROLLUP_PARAMS

        with open(rollup_params_file, "w") as f:
            json.dump(rollup_params, f)

        cmd.extend(["--rollup-params", rollup_params_file])

        props = {"rpc_port": rpc_port, "id": id}

        rpc_url = f"ws://localhost:{rpc_port}"

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()

        def _create_rpc():
            return seqrpc.JsonrpcClient(rpc_url)

        svc.create_rpc = _create_rpc

        return svc


class RethFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)

    @flexitest.with_ectx("ctx")
    def create_exec_client(
        self,
        id: int,
        reth_secret_path: str,
        sequencer_reth_rpc: Optional[str],
        ctx: flexitest.EnvContext,
    ) -> flexitest.Service:
        datadir = ctx.make_service_dir(f"reth.{id}")
        authrpc_port = self.next_port()
        listener_port = self.next_port()
        ethrpc_ws_port = self.next_port()
        ethrpc_http_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        # fmt: off
        cmd = [
            "strata-reth",
            "--disable-discovery",
            "--ipcdisable",
            "--datadir", datadir,
            "--authrpc.port", str(authrpc_port),
            "--authrpc.jwtsecret", reth_secret_path,
            "--port", str(listener_port),
            "--ws",
            "--ws.port", str(ethrpc_ws_port),
            "--http",
            "--http.port", str(ethrpc_http_port),
            "--color", "never",
            "--enable-witness-gen",
            # TODO: update tests to use new chain config
            "--custom-chain", "dev",
            "-vvvv"
        ]
        # fmt: on

        if sequencer_reth_rpc is not None:
            cmd.extend(["--sequencer-http", sequencer_reth_rpc])

        props = {"rpc_port": authrpc_port, "eth_rpc_http_port": ethrpc_http_port}

        ethrpc_url = f"ws://localhost:{ethrpc_ws_port}"

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()

        def _create_rpc():
            return seqrpc.JsonrpcClient(ethrpc_url)

        def _create_web3():
            http_ethrpc_url = f"http://localhost:{ethrpc_http_port}"
            w3 = web3.Web3(web3.Web3.HTTPProvider(http_ethrpc_url))
            # address, pk hardcoded in test genesis config
            w3.address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            account = w3.eth.account.from_key(
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            )
            w3.middleware_onion.add(web3.middleware.SignAndSendRawMiddlewareBuilder.build(account))
            return w3

        svc.create_rpc = _create_rpc
        svc.create_web3 = _create_web3

        return svc


class ProverClientFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)

    @flexitest.with_ectx("ctx")
    def create_prover_client(
        self,
        bitcoind_config: BitcoinRpcConfig,
        sequencer_url: str,
        reth_url: str,
        ctx: flexitest.EnvContext,
    ):
        datadir = ctx.make_service_dir("prover_client")
        logfile = os.path.join(datadir, "service.log")

        rpc_port = self.next_port()
        rpc_url = f"ws://localhost:{rpc_port}"

        # fmt: off
        cmd = [
            "strata-prover-client",
            "--rpc-port", str(rpc_port),
            "--sequencer-rpc", sequencer_url,
            "--reth-rpc", reth_url,
            "--bitcoind-url", bitcoind_config["bitcoind_sock"],
            "--bitcoind-user", bitcoind_config["bitcoind_user"],
            "--bitcoind-password", bitcoind_config["bitcoind_pass"],
        ]
        # fmt: on
        props = {"rpc_port": rpc_port}

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()

        def _create_rpc():
            return seqrpc.JsonrpcClient(rpc_url)

        svc.create_rpc = _create_rpc
        return svc


class BasicEnvConfig(flexitest.EnvConfig):
    def __init__(
        self,
        pre_generate_blocks: int = 0,
        rollup_params: Optional[dict] = None,
        auto_generate_blocks=True,
        enable_prover_client=False,
    ):
        self.pre_generate_blocks = pre_generate_blocks
        self.rollup_params = rollup_params
        self.auto_generate_blocks = auto_generate_blocks
        self.enable_prover_client = enable_prover_client
        super().__init__()

    def init(self, ctx: flexitest.EnvContext) -> flexitest.LiveEnv:
        btc_fac = ctx.get_factory("bitcoin")
        seq_fac = ctx.get_factory("sequencer")
        reth_fac = ctx.get_factory("reth")

        # reth needs some time to startup, start it first
        secret_dir = ctx.make_service_dir("secret")
        reth_secret_path = os.path.join(secret_dir, "jwt.hex")

        with open(reth_secret_path, "w") as file:
            file.write(generate_jwt_secret())

        reth = reth_fac.create_exec_client(0, reth_secret_path, None)
        reth_port = reth.get_prop("rpc_port")

        bitcoind = btc_fac.create_regtest_bitcoin()
        # wait for services to to startup
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS)

        brpc = bitcoind.create_rpc()

        walletname = bitcoind.get_prop("walletname")
        brpc.proxy.createwallet(walletname)

        seqaddr = brpc.proxy.getnewaddress()

        chunk_size = 500
        while self.pre_generate_blocks > 0:
            batch_size = min(self.pre_generate_blocks, 1000)

            # generate blocks in chunks to avoid timeout
            num_chunks = ceil(batch_size / chunk_size)
            for i in range(0, batch_size, chunk_size):
                chunk = int(i / chunk_size) + 1
                num_blocks = int(min(chunk_size, batch_size - (chunk - 1) * chunk_size))
                chunk = f"{chunk}/{num_chunks}"

                print(f"Pre generating {num_blocks} blocks to address {seqaddr}; chunk = {chunk}")
                brpc.proxy.generatetoaddress(chunk_size, seqaddr)

            self.pre_generate_blocks -= batch_size

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
        sequencer = seq_fac.create_sequencer(
            bitcoind_config, reth_config, seqaddr, self.rollup_params
        )

        # Need to wait for at least `genesis_l1_height` blocks to be generated.
        # Sleeping some more for safety
        if self.auto_generate_blocks:
            time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 10)

        svcs = {"bitcoin": bitcoind, "sequencer": sequencer, "reth": reth}

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

        return flexitest.LiveEnv(svcs)


class HubNetworkEnvConfig(flexitest.EnvConfig):
    def __init__(
        self,
        pre_generate_blocks: int = 0,
        rollup_params: Optional[dict] = None,
        auto_generate_blocks=True,
    ):
        self.pre_generate_blocks = pre_generate_blocks
        self.rollup_params = rollup_params
        self.auto_generate_blocks = auto_generate_blocks
        super().__init__()

    def init(self, ctx: flexitest.EnvContext) -> flexitest.LiveEnv:
        btc_fac = ctx.get_factory("bitcoin")
        seq_fac = ctx.get_factory("sequencer")
        reth_fac = ctx.get_factory("reth")
        fn_fac = ctx.get_factory("fullnode")

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
        sequencer = seq_fac.create_sequencer(
            bitcoind_config, reth_config, seqaddr, self.rollup_params
        )
        # Need to wait for at least `genesis_l1_height` blocks to be generated.
        # Sleeping some more for safety
        if self.auto_generate_blocks:
            time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 10)

        fullnode_reth_port = fullnode_reth.get_prop("rpc_port")
        fullnode_reth_config = {
            "reth_socket": f"localhost:{fullnode_reth_port}",
            "reth_secret_path": reth_secret_path,
        }

        sequencer_rpc = f"http://localhost:{sequencer.get_prop('rpc_port')}"

        fullnode = fn_fac.create_fullnode(
            bitcoind_config,
            fullnode_reth_config,
            sequencer_rpc,
            self.rollup_params,
        )

        svcs = {
            "bitcoin": bitcoind,
            "seq_node": sequencer,
            "seq_reth": reth,
            "follower_1_node": fullnode,
            "follower_1_reth": fullnode_reth,
        }
        return flexitest.LiveEnv(svcs)


def main(argv):
    test_dir = os.path.dirname(os.path.abspath(__file__))
    modules = flexitest.runtime.scan_dir_for_modules(test_dir)
    all_tests = flexitest.runtime.load_candidate_modules(modules)

    # Avoid running prover related tets while running all the tests
    # Filter the prover test files if not present in argv
    if len(argv) > 1:
        # Run the specific test file passed as the first argument (without .py extension)
        tests = [str(argv[1]).removesuffix(".py")]
    else:
        # Run all tests, excluding those containing "fn_prover", unless explicitly passed in argv
        tests = [test for test in all_tests if "fn_prover" not in test or test in argv]

    datadir_root = flexitest.create_datadir_in_workspace(os.path.join(test_dir, DD_ROOT))

    btc_fac = BitcoinFactory([12300 + i for i in range(30)])
    seq_fac = StrataFactory([12400 + i for i in range(30)])
    fullnode_fac = FullNodeFactory([12500 + i for i in range(30)])
    reth_fac = RethFactory([12600 + i for i in range(20 * 3)])
    prover_client_fac = ProverClientFactory([12700 + i for i in range(20 * 3)])

    factories = {
        "bitcoin": btc_fac,
        "sequencer": seq_fac,
        "fullnode": fullnode_fac,
        "reth": reth_fac,
        "prover_client": prover_client_fac,
    }

    global_envs = {
        "basic": BasicEnvConfig(),
        "premined_blocks": BasicEnvConfig(101),
        "fast_batches": BasicEnvConfig(101, rollup_params=FAST_BATCH_ROLLUP_PARAMS),
        "hub1": HubNetworkEnvConfig(),
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
