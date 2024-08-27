#!/usr/bin/env python3
import json
import logging as log
import os
import sys
import time
from threading import Thread
from typing import Optional

import flexitest
import web3
from bitcoinlib.services.bitcoind import BitcoindClient
import web3.middleware

import seqrpc
from constants import BD_PASSWORD, BD_USERNAME, BLOCK_GENERATION_INTERVAL_SECS, DD_ROOT


def generate_seqkey() -> bytes:
    # this is just for fun
    buf = b"alpen" + b"_1337" * 5 + b"xx"
    assert len(buf) == 32, "bad seqkey len"
    return buf


def generate_jwt_secret() -> str:
    return os.urandom(32).hex()


def generate_blocks(
    bitcoin_rpc: BitcoindClient,
    wait_dur,
    addr: str,
) -> Thread:
    thr = Thread(
        target=generate_task,
        args=(
            bitcoin_rpc,
            wait_dur,
            addr,
        ),
    )
    thr.start()
    return thr


def generate_task(rpc: BitcoindClient, wait_dur, addr):
    while True:
        time.sleep(wait_dur)
        try:
            rpc.proxy.generatetoaddress(1, addr)
        except Exception as ex:
            log.warning(f"{ex} while generating to address {addr}")
            return


def generate_n_blocks(bitcoin_rpc: BitcoindClient, n: int):
    addr = bitcoin_rpc.proxy.getnewaddress()
    print(f"generating {n} blocks to address", addr)
    try:
        blk = bitcoin_rpc.proxy.generatetoaddress(n, addr)
        print("made blocks", blk)
    except Exception as ex:
        log.warning(f"{ex} while generating address")
        return


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
        }

        with open(logfile, "w") as f:
            svc = flexitest.service.ProcService(props, cmd, stdout=f)

            def _create_rpc():
                url = f"http://{BD_USERNAME}:{BD_PASSWORD}@localhost:{rpc_port}"
                return BitcoindClient(base_url=url, network="regtest")

            svc.create_rpc = _create_rpc

            return svc


class ExpressFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)

    @flexitest.with_ectx("ctx")
    def create_sequencer(
        self,
        bitcoind_sock: str,
        bitcoind_user: str,
        bitcoind_pass: str,
        reth_socket: str,
        reth_secret_path: str,
        sequencer_address: str,
        rollup_params: Optional[dict],
        ctx: flexitest.EnvContext,
    ) -> flexitest.Service:
        datadir = ctx.make_service_dir("sequencer")
        rpc_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        keyfile = os.path.join(datadir, "seqkey.bin")
        seqkey = generate_seqkey()
        with open(keyfile, "wb") as f:
            f.write(seqkey)

        # fmt: off
        cmd = [
            "alpen-express-sequencer",
            "--datadir", datadir,
            "--rpc-port", str(rpc_port),
            "--bitcoind-host", bitcoind_sock,
            "--bitcoind-user", bitcoind_user,
            "--bitcoind-password", bitcoind_pass,
            "--reth-authrpc", reth_socket,
            "--reth-jwtsecret", reth_secret_path,
            "--network", "regtest",
            "--sequencer-key", keyfile,
            "--sequencer-bitcoin-address", sequencer_address,
        ]
        # fmt: on

        if rollup_params:
            rollup_params_file = os.path.join(datadir, "rollup_params.json")
            with open(rollup_params_file, "w") as f:
                json.dump(rollup_params, f)

            cmd.extend(["--rollup-params", rollup_params_file])

        props = {"rpc_port": rpc_port, "seqkey": seqkey, "address": sequencer_address}

        rpc_url = f"ws://localhost:{rpc_port}"

        with open(logfile, "w") as f:
            svc = flexitest.service.ProcService(props, cmd, stdout=f)

            def _create_rpc():
                return seqrpc.JsonrpcClient(rpc_url)

            svc.create_rpc = _create_rpc

            return svc


class RethFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)

    @flexitest.with_ectx("ctx")
    def create_exec_client(
        self, reth_secret_path: str, ctx: flexitest.EnvContext
    ) -> flexitest.Service:
        datadir = ctx.make_service_dir("reth")
        authrpc_port = self.next_port()
        listener_port = self.next_port()
        ethrpc_ws_port = self.next_port()
        ethrpc_http_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        # fmt: off
        cmd = [
            "alpen-express-reth",
            "--disable-discovery",
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
            "-vvvv"
        ]
        # fmt: on
        props = {"rpc_port": authrpc_port}

        ethrpc_url = f"ws://localhost:{ethrpc_ws_port}"

        with open(logfile, "w") as f:
            svc = flexitest.service.ProcService(props, cmd, stdout=f)

            def _create_rpc():
                return seqrpc.JsonrpcClient(ethrpc_url)
            
            def _create_web3():
                http_ethrpc_url = f"http://localhost:{ethrpc_http_port}"
                w3 = web3.Web3(web3.Web3.HTTPProvider(http_ethrpc_url))
                # address, pk hardcoded in test genesis config
                w3.address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
                account = w3.eth.account.from_key("0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80")
                w3.middleware_onion.add(web3.middleware.SignAndSendRawMiddlewareBuilder.build(account))
                return w3

            svc.create_rpc = _create_rpc
            svc.create_web3 = _create_web3

            return svc


class BasicEnvConfig(flexitest.EnvConfig):
    def __init__(self, pre_generate_blocks: int = 0, rollup_params: Optional[dict] = None):
        self.pre_generate_blocks = pre_generate_blocks
        self.rollup_params = rollup_params
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

        reth = reth_fac.create_exec_client(reth_secret_path)

        reth_port = reth.get_prop("rpc_port")
        reth_socket = f"localhost:{reth_port}"

        bitcoind = btc_fac.create_regtest_bitcoin()
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS)

        brpc = bitcoind.create_rpc()

        walletname = "dummy"
        brpc.proxy.createwallet(walletname)

        seqaddr = brpc.proxy.getnewaddress()

        if self.pre_generate_blocks > 0:
            print(f"Pre generating {self.pre_generate_blocks} blocks to address {seqaddr}")
            brpc.proxy.generatetoaddress(self.pre_generate_blocks, seqaddr)

        # generate blocks every 500 millis
        generate_blocks(brpc, BLOCK_GENERATION_INTERVAL_SECS, seqaddr)
        rpc_port = bitcoind.get_prop("rpc_port")
        rpc_user = bitcoind.get_prop("rpc_user")
        rpc_pass = bitcoind.get_prop("rpc_password")
        rpc_sock = f"localhost:{rpc_port}/wallet/{walletname}"
        sequencer = seq_fac.create_sequencer(
            rpc_sock, rpc_user, rpc_pass, reth_socket, reth_secret_path, seqaddr, self.rollup_params
        )
        # Need to wait for at least `genesis_l1_height` blocks to be generated.
        # Sleeping some more for safety
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 10)

        svcs = {"bitcoin": bitcoind, "sequencer": sequencer, "reth": reth}
        return flexitest.LiveEnv(svcs)


# post batch every 5 l2 blocks
FAST_BATCH_ROLLUP_PARAMS = {
    "rollup_name": "expresssss",
    "block_time": 1000,
    "cred_rule": "Unchecked",
    "horizon_l1_height": 3,
    "genesis_l1_height": 5,
    "evm_genesis_block_hash": "37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba",
    "evm_genesis_block_state_root": "351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f",  # noqa: E501
    "l1_reorg_safe_depth": 4,
    "batch_l2_blocks_target": 5,
}


def main(argv):
    test_dir = os.path.dirname(os.path.abspath(__file__))
    modules = flexitest.runtime.scan_dir_for_modules(test_dir)
    all_tests = flexitest.runtime.load_candidate_modules(modules)

    tests = [str(argv[1]).removesuffix(".py")] if len(argv) > 1 else all_tests

    datadir_root = flexitest.create_datadir_in_workspace(os.path.join(test_dir, DD_ROOT))

    btc_fac = BitcoinFactory([12300 + i for i in range(20)])
    seq_fac = ExpressFactory([12400 + i for i in range(20)])
    reth_fac = RethFactory([12500 + i for i in range(20 * 3)])

    factories = {"bitcoin": btc_fac, "sequencer": seq_fac, "reth": reth_fac}
    global_envs = {
        "basic": BasicEnvConfig(),
        "premined_blocks": BasicEnvConfig(101),
        "fast_batches": BasicEnvConfig(101, rollup_params=FAST_BATCH_ROLLUP_PARAMS),
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
