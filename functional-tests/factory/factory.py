import os
from typing import Optional, TypedDict

import flexitest
import web3
import web3.middleware
from bitcoinlib.services.bitcoind import BitcoindClient

from factory import seqrpc
from utils import *
from utils.constants import *


class BitcoinRpcConfig(TypedDict):
    bitcoind_sock: str
    bitcoind_user: str
    bitcoind_pass: str


class RethConfig(TypedDict):
    reth_socket: str
    reth_secret_path: str


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
            "-listen",
            f"-port={p2p_port}",
            "-printtoconsole",
            "-fallbackfee=0.00001",
            f"-datadir={datadir}",
            f"-rpcport={rpc_port}",
            f"-rpcuser={BD_USERNAME}",
            f"-rpcpassword={BD_PASSWORD}",
        ]

        props = {
            "p2p_port": p2p_port,
            "rpc_port": rpc_port,
            "rpc_user": BD_USERNAME,
            "rpc_password": BD_PASSWORD,
            "walletname": "testwallet",
        }

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()

        def _create_rpc():
            st = svc.check_status()
            if not st:
                raise RuntimeError("service isn't active")
            url = f"http://{BD_USERNAME}:{BD_PASSWORD}@localhost:{rpc_port}"
            return BitcoindClient(base_url=url, network="regtest")

        svc.create_rpc = _create_rpc

        return svc


class StrataFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)

    @flexitest.with_ectx("ctx")
    def create_sequencer(
        self,
        bitcoind_config: BitcoinRpcConfig,
        reth_config: RethConfig,
        sequencer_address: str,
        rollup_params: str,
        ctx: flexitest.EnvContext,
    ) -> flexitest.Service:
        datadir = ctx.make_service_dir("sequencer")
        rpc_port = self.next_port()
        rpc_host = "127.0.0.1"
        logfile = os.path.join(datadir, "service.log")

        seqkey_path = os.path.join(ctx.envdd_path, "_init", "seqkey.bin")
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
            "--sequencer-key", seqkey_path,
            "--sequencer-bitcoin-address", sequencer_address,
        ]
        # fmt: on

        rollup_params_file = os.path.join(datadir, "rollup_params.json")
        with open(rollup_params_file, "w") as f:
            f.write(rollup_params)

        cmd.extend(["--rollup-params", rollup_params_file])

        rpc_url = f"ws://{rpc_host}:{rpc_port}"
        props = {
            "rpc_host": rpc_host,
            "rpc_port": rpc_port,
            "rpc_url": rpc_url,
            "seqkey": seqkey_path,
            "address": sequencer_address,
        }

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()
        _inject_service_create_rpc(svc, rpc_url, "sequencer")
        return svc


# TODO merge with `StrataFactory` to reuse most of the init steps
class FullNodeFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)
        self._next_idx = 1

    def next_idx(self) -> int:
        idx = self._next_idx
        self._next_idx += 1
        return idx

    @flexitest.with_ectx("ctx")
    def create_fullnode(
        self,
        bitcoind_config: BitcoinRpcConfig,
        reth_config: RethConfig,
        sequencer_rpc: str,
        rollup_params: str,
        ctx: flexitest.EnvContext,
    ) -> flexitest.Service:
        idx = self.next_idx()
        name = f"fullnode.{idx}"

        datadir = ctx.make_service_dir(name)
        rpc_host = "127.0.0.1"
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
        with open(rollup_params_file, "w") as f:
            f.write(rollup_params)

        cmd.extend(["--rollup-params", rollup_params_file])

        rpc_url = f"ws://localhost:{rpc_port}"
        props = {
            "id": idx,
            "rpc_port": rpc_port,
            "rpc_url": rpc_url,
        }

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()
        _inject_service_create_rpc(svc, rpc_url, name)
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
        custom_chain: str = "dev",
    ) -> flexitest.Service:
        name = f"reth.{id}"
        datadir = ctx.make_service_dir(name)
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
            "--custom-chain", custom_chain,
            "-vvvv"
        ]
        # fmt: on

        if sequencer_reth_rpc is not None:
            cmd.extend(["--sequencer-http", sequencer_reth_rpc])

        props = {"rpc_port": authrpc_port, "eth_rpc_http_port": ethrpc_http_port}

        ethrpc_url = f"ws://localhost:{ethrpc_ws_port}"

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()

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

        _inject_service_create_rpc(svc, ethrpc_url, name)
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
        rollup_params: str,
        settings: ProverClientSettings,
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
            "--datadir", datadir,
            "--native-workers", str(settings.native_workers),
            "--loop-interval", str(settings.loop_interval)
        ]
        # fmt: on

        rollup_params_file = os.path.join(datadir, "rollup_params.json")
        with open(rollup_params_file, "w") as f:
            f.write(rollup_params)

        cmd.extend(["--rollup-params", rollup_params_file])

        props = {"rpc_port": rpc_port}

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()
        _inject_service_create_rpc(svc, rpc_url, "prover")
        return svc


class BridgeClientFactory(flexitest.Factory):
    def __init__(self, port_range: list[int]):
        super().__init__(port_range)
        self._next_idx = 1

    def next_idx(self) -> int:
        idx = self._next_idx
        self._next_idx += 1
        return idx

    @flexitest.with_ectx("ctx")
    def create_operator(
        self,
        master_xpriv: str,
        node_url: str,
        bitcoind_config: dict,
        ctx: flexitest.EnvContext,
        message_interval: int,
        duty_timeout_duration: int,
    ):
        idx = self.next_idx()
        name = f"bridge.{idx}"
        datadir = ctx.make_service_dir(name)
        rpc_host = "127.0.0.1"
        rpc_port = self.next_port()
        logfile = os.path.join(datadir, "service.log")

        # fmt: off
        cmd = [
            "strata-bridge-client",
            "operator",
            "--datadir", datadir,
            "--master-xpriv", master_xpriv,
            "--rpc-host", rpc_host,
            "--rpc-port", str(rpc_port),
            "--btc-url", "http://" + bitcoind_config["bitcoind_sock"],
            "--btc-user", bitcoind_config["bitcoind_user"],
            "--btc-pass", bitcoind_config["bitcoind_pass"],
            "--rollup-url", node_url,
            "--message-interval", str(message_interval),
            "--duty-timeout-duration", str(duty_timeout_duration),
        ]
        # fmt: on

        # TODO add a way to expose this
        # TODO remove this after adding a proper config file
        # ruff: noqa: F841
        envvars = {
            "STRATA_OP_MASTER_XPRIV": master_xpriv,
        }

        props = {"id": idx, "rpc_host": rpc_host, "rpc_port": rpc_port}
        rpc_url = f"ws://localhost:{rpc_port}"

        svc = flexitest.service.ProcService(props, cmd, stdout=logfile)
        svc.start()
        _inject_service_create_rpc(svc, rpc_url, name)
        return svc


def _inject_service_create_rpc(svc: flexitest.service.ProcService, rpc_url: str, name: str):
    """
    Injects a `create_rpc` method using JSON-RPC onto a `ProcService`, checking
    its status before each call.
    """

    def _status_ck(method: str):
        """
        Hook to check that the process is still running before every call.
        """
        if not svc.check_status():
            print(f"service '{name}' seems to have crashed as of call to {method}")
            raise RuntimeError(f"process '{name}' crashed")

    def _create_rpc():
        rpc = seqrpc.JsonrpcClient(rpc_url)
        rpc._pre_call_hook = _status_ck
        return rpc

    svc.create_rpc = _create_rpc
