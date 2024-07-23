#!/usr/bin/env python3

import logging as log
import os
import secrets
import string
import sys
import time
from threading import Thread

import flexitest
from bitcoinlib.services.bitcoind import BitcoindClient

import seqrpc

BD_USERNAME = "alpen"
BD_PASSWORD = "alpen"
LETTERS = string.ascii_lowercase
DATADIR_NAME_LENGTH = 10


def generate_seqkey() -> bytes:
    # this is just for fun
    buf = b"alpen" + b"_1337" * 5 + b"xx"
    assert len(buf) == 32, "bad seqkey len"
    return buf


def generate_blocks(
    bitcoin_rpc: BitcoindClient,
    wait_dur,
) -> Thread:
    addr = bitcoin_rpc.proxy.getnewaddress()
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
    print("generating to address", addr)
    while True:
        time.sleep(wait_dur)
        try:
            blk = rpc.proxy.generatetoaddress(1, addr)
            print("made block", blk)
        except Exception as ex:
            log.warning(f"{ex} while generating address")
            return


class BitcoinFactory(flexitest.Factory):
    def __init__(self, datadir_pfx: str, port_range: list[int]):
        super().__init__(datadir_pfx, port_range)

    def create_regtest_bitcoin(self) -> flexitest.Service:
        datadir_name = "".join(secrets.choice(LETTERS) for i in range(DATADIR_NAME_LENGTH))
        log.warning(f"Using {datadir_name} as the datadir for bitcoin regtest")
        datadir = self.create_datadir(f"bitcoin_{datadir_name}")
        p2p_port = self.next_port()
        rpc_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        cmd = [
            "bitcoind",
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
                return BitcoindClient(base_url=url)

            svc.create_rpc = _create_rpc

            return svc


class VertexFactory(flexitest.Factory):
    def __init__(self, datadir_pfx: str, port_range: list[int]):
        super().__init__(datadir_pfx, port_range)

    def create_sequencer(
        self, bitcoind_sock: str, bitcoind_user: str, bitcoind_pass: str
    ) -> flexitest.Service:
        datadir_name = "".join(secrets.choice(LETTERS) for i in range(DATADIR_NAME_LENGTH))
        log.warning(f"Using {datadir_name} as the datadir for sequencer")
        datadir = self.create_datadir(f"seq_{datadir_name}")
        rpc_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        keyfile = os.path.join(datadir, "seqkey.bin")
        seqkey = generate_seqkey()
        with open(keyfile, "wb") as f:
            f.write(seqkey)

        # TODO EL setup, this is actually two services running coupled

        # fmt: off
        cmd = [
            "alpen-vertex-sequencer",
            "--datadir", datadir,
            "--rpc-port", str(rpc_port),
            "--bitcoind-host", bitcoind_sock,
            "--bitcoind-user", bitcoind_user,
            "--bitcoind-password", bitcoind_pass,
            "--network",
            "regtest",
            "--sequencer-key", keyfile,
        ]
        # fmt: on
        props = {"rpc_port": rpc_port, "seqkey": seqkey}

        rpc_url = f"ws://localhost:{rpc_port}"

        with open(logfile, "w") as f:
            svc = flexitest.service.ProcService(props, cmd, stdout=f)

            def _create_rpc():
                return seqrpc.JsonrpcClient(rpc_url)

            svc.create_rpc = _create_rpc

            return svc


class BasicEnvConfig(flexitest.EnvConfig):
    def __init__(self):
        pass

    def init(self, facs: dict) -> flexitest.LiveEnv:
        btc_fac = facs["bitcoin"]
        seq_fac = facs["sequencer"]

        bitcoind = btc_fac.create_regtest_bitcoin()
        time.sleep(0.5)

        brpc = bitcoind.create_rpc()
        brpc.proxy.createwallet("dummy")

        # generate blocks every 500 millis
        generate_blocks(brpc, 0.5)
        rpc_port = bitcoind.get_prop("rpc_port")
        rpc_user = bitcoind.get_prop("rpc_user")
        rpc_pass = bitcoind.get_prop("rpc_password")
        rpc_sock = f"localhost:{rpc_port}"
        sequencer = seq_fac.create_sequencer(rpc_sock, rpc_user, rpc_pass)
        time.sleep(0.5)

        svcs = {"bitcoin": bitcoind, "sequencer": sequencer}
        return flexitest.LiveEnv(svcs)


def main(argv):
    test_dir = os.path.dirname(os.path.abspath(__file__))
    modules = flexitest.runtime.scan_dir_for_modules(test_dir)

    tests = [argv[1]] if len(argv) > 1 else flexitest.runtime.load_candidate_modules(modules)

    for test in tests:
        datadir_root = flexitest.create_datadir_in_workspace(os.path.join(test_dir, "_dd"))

        btc_fac = BitcoinFactory(datadir_root, [12300 + i for i in range(20)])
        seq_fac = VertexFactory(datadir_root, [12400 + i for i in range(20)])

        factories = {"bitcoin": btc_fac, "sequencer": seq_fac}
        envs = {"basic": BasicEnvConfig()}

        rt = flexitest.TestRuntime(envs, datadir_root, factories)
        rt.prepare_registered_tests()

        results = rt.run_tests([test])
        flexitest.dump_results(results)

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
