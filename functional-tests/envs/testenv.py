import time
from math import ceil
from typing import Optional

import flexitest
from strata_utils import (
    deposit_request_transaction,
    extract_p2tr_pubkey,
    get_address,
    get_recovery_address,
)
from web3 import Web3, middleware

from envs.rollup_params_cfg import RollupConfig
from utils import *
from utils.constants import *

# Local constants
# Ethereum Private Key
# NOTE: don't use this private key in production
ETH_PRIVATE_KEY = "0x0000000000000000000000000000000000000000000000000000000000000001"


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


class BridgeTestBase(StrataTester):
    """
    Testbase for bridge specific test.
    Provides methods for setting up service, making DRT, withdraw transaction
    """

    def premain(self, ctx: flexitest.RunContext):
        super().premain(ctx)
        self.btc = ctx.get_service("bitcoin")
        self.seq = ctx.get_service("sequencer")
        self.reth = ctx.get_service("reth")

        self.seqrpc = self.seq.create_rpc()
        self.btcrpc: BitcoindClient = self.btc.create_rpc()
        self.rethrpc = self.reth.create_rpc()

        self.web3: Web3 = self.reth.create_web3()
        self.eth_account = self.web3.eth.account.from_key(ETH_PRIVATE_KEY)

        # Inject signing middleware
        self.web3.middleware_onion.inject(
            middleware.SignAndSendRawMiddlewareBuilder.build(self.eth_account),
            layer=0,
        )

    def deposit(self, ctx: flexitest.RunContext, el_address, bridge_pk):
        """
        Make two DRT deposits to ensure the EL address has enough funds for gas
        and for subsequent withdrawals. Wait until the deposit is reflected on L2.
        """
        cfg: RollupConfig = ctx.env.rollup_cfg()
        # D BTC
        deposit_amount = cfg.deposit_amount

        # bridge pubkey
        self.debug(f"Bridge pubkey: {bridge_pk}")

        # check balance before deposit
        initial_balance = int(self.rethrpc.eth_getBalance(el_address), 16)
        self.debug(f"Strata Balance right before deposit calls: {initial_balance}")

        self.make_drt(ctx, el_address, bridge_pk)

        # Wait until the deposit is seen on L2
        expected_balance = initial_balance + deposit_amount * SATS_TO_WEI
        wait_until(
            lambda: int(self.rethrpc.eth_getBalance(el_address), 16) == expected_balance,
            error_with="Strata balance after deposit is not as expected",
        )

    def withdraw(
        self,
        ctx: flexitest.RunContext,
        el_address: str,
        withdraw_address: str,
    ):
        """
        Perform a withdrawal from the L2 to the given BTC withdraw address.
        Returns (l2_tx_hash, tx_receipt, total_gas_used).
        """
        cfg: RollupConfig = ctx.env.rollup_cfg()
        # D BTC
        deposit_amount = cfg.deposit_amount
        # Build the p2tr pubkey from the withdraw address
        change_address_pk = extract_p2tr_pubkey(withdraw_address)
        self.debug(f"Change Address PK: {change_address_pk}")

        # Estimate gas
        estimated_withdraw_gas = self.__estimate_withdraw_gas(
            deposit_amount, el_address, change_address_pk
        )
        self.debug(f"Estimated withdraw gas: {estimated_withdraw_gas}")

        l2_tx_hash = self.__make_withdraw(
            deposit_amount, el_address, change_address_pk, estimated_withdraw_gas
        ).hex()
        self.debug(f"Sent withdrawal transaction with hash: {l2_tx_hash}")

        # Wait for transaction receipt
        tx_receipt = wait_until_with_value(
            lambda: self.web3.eth.get_transaction_receipt(l2_tx_hash),
            predicate=lambda v: v is not None,
        )
        self.debug(f"Transaction receipt: {tx_receipt}")

        total_gas_used = tx_receipt["gasUsed"] * tx_receipt["effectiveGasPrice"]
        self.debug(f"Total gas used: {total_gas_used}")

        # Ensure the leftover in the EL address is what's expected (deposit minus gas)
        balance_post_withdraw = int(self.rethrpc.eth_getBalance(el_address), 16)
        difference = deposit_amount * SATS_TO_WEI - total_gas_used
        self.debug(f"Strata Balance after withdrawal: {balance_post_withdraw}")
        self.debug(f"Strata Balance difference: {difference}")
        assert difference == balance_post_withdraw, "balance difference is not expected"

        return l2_tx_hash, tx_receipt, total_gas_used

    def __make_withdraw(
        self,
        deposit_amount,
        el_address,
        change_address_pk,
        gas,
    ):
        """
        Withdrawal Request Transaction in Strata's EVM.
        """
        data_bytes = bytes.fromhex(change_address_pk)

        transaction = {
            "from": el_address,
            "to": PRECOMPILE_BRIDGEOUT_ADDRESS,
            "value": deposit_amount * SATS_TO_WEI,
            "gas": gas,
            "data": data_bytes,
        }
        l2_tx_hash = self.web3.eth.send_transaction(transaction)
        return l2_tx_hash

    def __estimate_withdraw_gas(self, deposit_amount, el_address, change_address_pk):
        """
        Estimate the gas for the withdrawal transaction.
        """

        data_bytes = bytes.fromhex(change_address_pk)

        transaction = {
            "from": el_address,
            "to": PRECOMPILE_BRIDGEOUT_ADDRESS,
            "value": deposit_amount * SATS_TO_WEI,
            "data": data_bytes,
        }
        return self.web3.eth.estimate_gas(transaction)

    def make_drt(self, ctx: flexitest.RunContext, el_address, musig_bridge_pk):
        """
        Deposit Request Transaction
        """
        # Get relevant data
        btc_url = self.btcrpc.base_url
        btc_user = self.btc.get_prop("rpc_user")
        btc_password = self.btc.get_prop("rpc_password")
        seq_addr = self.seq.get_prop("address")

        # Create the deposit request transaction
        tx = bytes(
            deposit_request_transaction(
                el_address, musig_bridge_pk, btc_url, btc_user, btc_password
            )
        ).hex()

        # Send the transaction to the Bitcoin network
        self.btcrpc.proxy.sendrawtransaction(tx)
        time.sleep(1)

        # time to mature DRT
        self.btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)

        # time to mature DT
        self.btcrpc.proxy.generatetoaddress(6, seq_addr)
        time.sleep(3)


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
        auto_generate_blocks: bool = True,
        pre_fund_addrs: bool = True,
        n_operators: int = 2,
        message_interval: int = 0,
        duty_timeout_duration: int = 10,
        custom_chain: str = "dev",
    ):
        super().__init__()
        self.pre_generate_blocks = pre_generate_blocks
        self.rollup_settings = rollup_settings
        self.auto_generate_blocks = auto_generate_blocks
        self.pre_fund_addrs = pre_fund_addrs
        self.n_operators = n_operators
        self.message_interval = message_interval
        self.duty_timeout_duration = duty_timeout_duration
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

        operator_message_interval = self.message_interval or settings.message_interval
        # Create all the bridge clients.
        for i in range(self.n_operators):
            xpriv_path = params_gen_data["opseedpaths"][i]
            xpriv = None
            with open(xpriv_path) as f:
                xpriv = f.read().strip()
            seq_url = sequencer.get_prop("rpc_url")
            br = bridge_fac.create_operator(
                xpriv,
                seq_url,
                bitcoind_config,
                message_interval=operator_message_interval,
                duty_timeout_duration=self.duty_timeout_duration,
            )
            name = f"bridge.{i}"
            svcs[name] = br

        seq_port = sequencer.get_prop("rpc_port")
        reth_rpc_http_port = reth.get_prop("eth_rpc_http_port")

        prover_client_fac = ctx.get_factory("prover_client")
        prover_client = prover_client_fac.create_prover_client(
            bitcoind_config,
            f"http://localhost:{seq_port}",
            f"http://localhost:{reth_rpc_http_port}",
            params,
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
            br = bridge_fac.create_operator(
                xpriv,
                seq_url,
                bitcoind_config,
                message_interval=settings.message_interval,
                duty_timeout_duration=self.duty_timeout_duration,
            )
            name = f"bridge.{i}"
            svcs[name] = br

        return BasicLiveEnv(svcs, bridge_pk, rollup_cfg)
