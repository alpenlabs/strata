#!/usr/bin/env python3
import logging as log
import os
import sys
import time
from threading import Thread

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

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
        ctx: flexitest.EnvContext,
    ) -> flexitest.Service:
        datadir = ctx.make_service_dir("sequencer")
        rpc_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        keyfile = os.path.join(datadir, "seqkey.bin")
        seqkey = generate_seqkey()
        with open(keyfile, "wb") as f:
            f.write(seqkey)

        # TODO EL setup, this is actually two services running coupled

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
        rpc_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        # fmt: off
        cmd = [
            "alpen-express-reth",
            "--datadir", datadir,
            "--authrpc.port", str(rpc_port),
            "--authrpc.jwtsecret", reth_secret_path,
            "-vvvv"
        ]
        # fmt: on
        props = {"rpc_port": rpc_port}

        with open(logfile, "w") as f:
            svc = flexitest.service.ProcService(props, cmd, stdout=f)

            return svc


class BasicEnvConfig(flexitest.EnvConfig):
    def __init__(self, pre_generate_blocks: int = 0):
        self.pre_generate_blocks = pre_generate_blocks
        super().__init__()

    def init(self, ctx: flexitest.EnvContext) -> flexitest.LiveEnv:
        btc_fac = ctx.get_factory("bitcoin")
        seq_fac = ctx.get_factory("sequencer")
        reth_fac = ctx.get_factory("reth")

        bitcoind = btc_fac.create_regtest_bitcoin()
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS)

        brpc = bitcoind.create_rpc()

        walletname = "dummy"
        brpc.proxy.createwallet(walletname)

        seqaddr = brpc.proxy.getnewaddress()

        if self.pre_generate_blocks > 0:
            print(f"Pre generating {self.pre_generate_blocks} blocks to address {seqaddr}")
            brpc.proxy.generatetoaddress(self.pre_generate_blocks, seqaddr)

        secret_dir = ctx.make_service_dir("secret")
        reth_secret_path = os.path.join(secret_dir, "jwt.hex")

        with open(reth_secret_path, "w") as file:
            file.write(generate_jwt_secret())

        reth = reth_fac.create_exec_client(reth_secret_path)

        reth_port = reth.get_prop("rpc_port")
        reth_socket = f"localhost:{reth_port}"

        # generate blocks every 500 millis
        generate_blocks(brpc, BLOCK_GENERATION_INTERVAL_SECS, seqaddr)
        rpc_port = bitcoind.get_prop("rpc_port")
        rpc_user = bitcoind.get_prop("rpc_user")
        rpc_pass = bitcoind.get_prop("rpc_password")
        rpc_sock = f"localhost:{rpc_port}/wallet/{walletname}"
        sequencer = seq_fac.create_sequencer(
            rpc_sock, rpc_user, rpc_pass, reth_socket, reth_secret_path, seqaddr
        )
        # Need to wait for at least `genesis_l1_height` blocks to be generated.
        # Sleeping some more for safety
        time.sleep(BLOCK_GENERATION_INTERVAL_SECS * 10)

        svcs = {"bitcoin": bitcoind, "sequencer": sequencer, "reth": reth}
        return flexitest.LiveEnv(svcs)


def main(argv):
    test_dir = os.path.dirname(os.path.abspath(__file__))
    modules = flexitest.runtime.scan_dir_for_modules(test_dir)
    all_tests = flexitest.runtime.load_candidate_modules(modules)

    tests = [str(argv[1]).removesuffix(".py")] if len(argv) > 1 else all_tests

    datadir_root = flexitest.create_datadir_in_workspace(os.path.join(test_dir, DD_ROOT))

    btc_fac = BitcoinFactory([12300 + i for i in range(20)])
    seq_fac = ExpressFactory([12400 + i for i in range(20)])
    reth_fac = RethFactory([12500 + i for i in range(20)])

    factories = {"bitcoin": btc_fac, "sequencer": seq_fac, "reth": reth_fac}
    global_envs = {"basic": BasicEnvConfig(), "premined_blocks": BasicEnvConfig(101)}

    rt = flexitest.TestRuntime(global_envs, datadir_root, factories)
    rt.prepare_registered_tests()

    results = rt.run_tests(tests)
    rt.save_json_file("results.json", results)
    flexitest.dump_results(results)

    flexitest.fail_on_error(results)

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
