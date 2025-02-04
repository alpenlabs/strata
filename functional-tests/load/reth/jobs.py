import solcx
from gevent.lock import Semaphore
from locust import task

from .reth import BaseRethLoadJob, TransactionType

SOL_VERSION = "0.8.0"
solcx.install_solc(SOL_VERSION)
CONTRACTS_DIR = "load/reth/contracts/"


def get_contract(compiled_sol, contract_name):
    for ct_id, ct_interface in compiled_sol.items():
        if ct_id.endswith(contract_name):
            return ct_interface["abi"], ct_interface["bin"]
    return None


def compile_contract(filename, contract_name=None):
    if contract_name is None:
        contract_name = filename.split(".")[0]

    compiled_sol = solcx.compile_files(
        [f"{CONTRACTS_DIR}{filename}"],
        output_values=["abi", "bin"],
        solc_version=SOL_VERSION,
    )
    return get_contract(compiled_sol, contract_name)


class EthJob(BaseRethLoadJob):
    def on_start(self):
        super().on_start()

        w3, new_acc = self.w3_with_new_acc()
        self.w3 = w3
        self.new_acc = new_acc

        b = self._balance(new_acc.address)
        self.c = 0
        print(f"BALANCE AFTER START: {b}")

    @task
    def get_block(self):
        self.c += 1

        if self.c % 10 != 0:
            return

        # print("GET_BLOCK REQUEST")
        for i in range(1, 20):
            block = self.w3.eth.get_block(hex(i))
            num = block["number"]
            txn_cnt = len(block["transactions"])
            hash = block["hash"]
            print(f"BLOCK DATA \t\t\t\t\t {hash}, {num}, {txn_cnt}")

    # @task
    # def block_num(self):
    #    print("BLOCK_NUM REQUEST")
    #    # Pure json-rpc without web3 with middleware.
    #    method = "eth_blockNumber"
    #    params = []
    #    payload = {"jsonrpc": "2.0", "method": method, "params": params, "id": 1}
    #    headers = {"Content-type": "application/json"}
    #    # response = session.post(self.host, json=payload, headers=headers)
    #    response = self.client.post("", json=payload, headers=headers)
    #    # print(f"raw json response: {response.json()}")
    #    print("BLOCK_NUMBER: {}".format(response.json()["result"]))

    # @task(5)
    # def send(self):
    #    print("TRANSFER TRANSACTION")
    #
    #    source = self.w3.address
    #    dest = self.w3.to_checksum_address("0x0000000000000000000000000000000000000001")
    #    to_transfer = 1_000_000_000_000_000_000
    #    try:
    #        tx_hash = self.w3.eth.send_transaction(
    #            {"to": dest, "value": hex(to_transfer), "gas": hex(100000), "from": source}
    #        )
    #        print(f"transfer transaction hash: {tx_hash}")
    #    except Exception as e:
    #        print(e)


class EthTransactions(BaseRethLoadJob):
    def before_start(self):
        super().before_start()

        w3, acc = self.w3_with_new_acc()
        self.w3 = w3
        self.acc = acc

        assert w3.is_connected(), "Web3 is not connected"

    def after_start(self):
        super().after_start()

        self._nonce = self.w3.eth.get_transaction_count(self.acc.address)
        self._nonce_lock = Semaphore()

        print("Deploying counter contract")
        counter_contract_address, counter_abi = self._deploy("Counter.sol", "Counter")
        print("Counter Contract Deployed at:", counter_contract_address)
        self.counter_contract_address = counter_contract_address
        self.counter_abi = counter_abi

        print("Deploying EGM contract")
        egm_contract_address, egm_abi = self._deploy("ERC20.sol", "ERC20", "EndGameMoney", "EGM")
        print("EGM Contract Deployed at:", egm_contract_address)
        self.egm_contract_address = egm_contract_address
        self.egm_abi = egm_abi

        print("Deploying SUSD contract")
        susd_contract_address, susd_abi = self._deploy("ERC20.sol", "ERC20", "StrataUSD", "SUSD")
        self.susd_contract_address = susd_contract_address
        self.susd_abi = susd_abi
        print("SUSD Contract Deployed at:", susd_contract_address)

        print("Deploying Uniswap Factory")
        uniswap_factory_address, uniswap_factory_abi = self._deploy("Uniswap.sol", "UniswapFactory")
        self.uniswap_factory_address = uniswap_factory_address
        self.uniswap_factory_abi = uniswap_factory_abi
        print("Uniswap Factory Contract Deployed at:", uniswap_factory_address)

        print("Deploying Uniswap Router")
        uniswap_router_address, uniswap_router_abi = self._deploy(
            "Uniswap.sol",
            "UniswapRouter",
            self.w3.to_checksum_address(uniswap_factory_address),
        )
        self.uniswap_router_address = uniswap_router_address
        self.uniswap_router_abi = uniswap_router_abi
        print("Uniswap Router Contract Deployed at:", uniswap_router_address)

        print(
            "Mint EGM: ",
            self.mint_erc20(self.egm_contract_address, self.egm_abi, self.acc.address, 1_000_000),
        )

        print(
            "Mint SUSD: ",
            self.mint_erc20(self.susd_contract_address, self.susd_abi, self.acc.address, 1_000_000),
        )

        print(
            "Approves1: ",
            self.approve(egm_contract_address, egm_abi, uniswap_router_address, 1_000_000),
        )

        print(
            "Approves2: ",
            self.approve(susd_contract_address, susd_abi, uniswap_router_address, 1_000_000),
        )

        print(
            "Add liquidity: ",
            self.add_liquidity(
                uniswap_router_address,
                uniswap_router_abi,
                egm_contract_address,
                susd_contract_address,
                100_000,
                100_000,
            ),
        )

        return
        # We either have a bug in our reth, or swap itself contains some neat bug.
        print(
            "Swap tokens: ",
            self.swap_tokens(
                uniswap_router_address,
                uniswap_router_abi,
                egm_contract_address,
                susd_contract_address,
                5_000,
            ),
        )

    def _send_raw_transaction(self, tx, tx_type: TransactionType):
        self._ensure_tx(tx, tx_type)
        signed_tx = self.acc.sign_transaction(tx)
        try:
            tx_hash = self.w3.eth.send_raw_transaction(signed_tx.raw_transaction)
            return self.w3.to_hex(tx_hash)
        except Exception as e:
            print(e)
            return None

    def _send_tx(self, tx):
        signed_tx = self.acc.sign_transaction(tx)
        try:
            tx_hash = self.w3.eth.send_raw_transaction(signed_tx.raw_transaction)
            return self.w3.to_hex(tx_hash)
        except Exception as e:
            print(e)
            return None

    def _send_signed_transaction(self, tx, tx_type: TransactionType):
        self._ensure_tx(tx, tx_type)
        try:
            tx_hash = self.w3.eth.send_transaction(tx)
        except Exception as e:
            print(e)
        return self.w3.to_hex(tx_hash)

    def _ensure_tx(
        self, tx, tx_type: TransactionType = TransactionType.LEGACY, is_rpc: bool = True
    ):
        tx.setdefault("from", self.acc.address)
        tx.setdefault("nonce", self._inc_nonce())

        if tx_type == TransactionType.LEGACY:
            tx.setdefault(
                "gasPrice", self.w3.eth.gas_price if is_rpc else self.w3.to_wei("1", "gwei")
            )
        elif tx_type == TransactionType.EIP2930:
            tx.setdefault("type", "0x1")
            tx.setdefault("chainId", self.w3.eth.chain_id)
            # Define an empty access_list for simplicity for now.
            tx.setdefault("accessList", [{"address": tx["to"], "storageKeys": []}])

            tx.setdefault(
                "gasPrice", self.w3.eth.gas_price if is_rpc else self.w3.to_wei("1", "gwei")
            )

        elif tx_type == TransactionType.EIP1559:
            tx.setdefault("type", "0x2")
            tx.setdefault("chainId", self.w3.eth.chain_id)

            # TODO: use is_rpc to fetch fee market if needed.
            tx.setdefault("maxPriorityFeePerGas", self.w3.to_wei("1", "gwei"))
            tx.setdefault("maxFeePerGas", self.w3.to_wei("2", "gwei"))

    def _inc_nonce(self):
        with self._nonce_lock:
            nonce = self._nonce
            self._nonce += 1
            return nonce

    # 1. Legacy Transaction
    def send_legacy_transaction(self, to, value):
        tx = {
            "to": to,
            "value": self.w3.to_wei(value, "ether"),
            "gas": 21000,
        }
        return self._send_raw_transaction(tx, TransactionType.LEGACY)

    # 2. EIP-1559 Transaction
    def send_eip1559_transaction(self, to, value):
        tx = {
            "to": to,
            "value": self.w3.to_wei(value, "ether"),
            "gas": 21000,
        }
        return self._send_raw_transaction(tx, TransactionType.EIP1559)

    # 3. EIP-2930 Transaction (Access List)
    def send_eip2930_transaction(self, to, value):
        tx = {
            "to": to,
            "value": self.w3.to_wei(value, "ether"),
            "gas": 21000,
        }
        return self._send_raw_transaction(tx, TransactionType.EIP2930)

    def transfer(self, to, value, tx_type: TransactionType):
        tx = {
            "to": to,
            "value": self.w3.to_wei(value, "ether"),
            "gas": 210000,
        }
        return self._send_signed_transaction(tx, tx_type)

    def call_contract(self, contract_address, abi, function_name, *args):
        try:
            contract = self.w3.eth.contract(address=contract_address, abi=abi)
            tx = {
                "gas": 1000000,
            }
            self._ensure_tx(tx)
            function_tx = contract.functions[function_name](*args).build_transaction(tx)
            tx_hash = self._send_tx(function_tx)
            # receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash)
            # print(receipt["status"])
            return tx_hash
        except Exception as e:
            print(e)

    def call_contract_wait(self, contract_address, abi, function_name, *args):
        try:
            contract = self.w3.eth.contract(address=contract_address, abi=abi)
            tx = {
                "gas": 1000000,
            }
            self._ensure_tx(tx)
            function_tx = contract.functions[function_name](*args).build_transaction(tx)
            tx_hash = self._send_tx(function_tx)
            receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash)
            print(receipt["status"])
            return tx_hash
        except Exception as e:
            print(e)

    def approve(self, contract_address, abi, spender, amount):
        return self.call_contract_wait(
            contract_address, abi, "approve", self.w3.to_checksum_address(spender), amount
        )

    def mint_erc20(self, contract_address, abi, to_address, amount):
        return self.call_contract(
            contract_address, abi, "mint", self.w3.to_checksum_address(to_address), amount
        )

    def add_liquidity(self, router_address, router_abi, tokenA, tokenB, amountA, amountB):
        return self.call_contract(
            router_address,
            router_abi,
            "addLiquidity",
            tokenA,
            tokenB,
            amountA,
            amountB,
        )

    def swap_tokens(self, router_address, router_abi, tokenIn, tokenOut, amountIn):
        return self.call_contract(router_address, router_abi, "swap", tokenIn, tokenOut, amountIn)

    def _deploy(
        self,
        contract_path,
        contract_name,
        *args,
    ):
        abi, bytecode = compile_contract(contract_path, contract_name)
        contract = self.w3.eth.contract(abi=abi, bytecode=bytecode)

        tx = {
            "gas": 5000000,
        }
        self._ensure_tx(tx)

        tx = contract.constructor(*args).build_transaction(tx)
        tx_hash = self._send_tx(tx)
        receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash)
        return receipt.contractAddress, abi

    @task
    def transactions_task(self):
        target_address = self.w3.eth.account.create().address
        # print(
        #    "Sending Legacy Transaction:",
        (self.transfer(target_address, 0.1, TransactionType.LEGACY),)
        # )

        # print(
        #    "Sending 2930 Transaction:",
        (self.transfer(target_address, 0.1, TransactionType.EIP2930),)
        # )

        # print(
        #    "Sending 1559 Transaction:",
        (self.transfer(target_address, 0.1, TransactionType.EIP1559),)
        # )

        # print(
        #    "Incrementing Counter:",
        (self.call_contract(self.counter_contract_address, self.counter_abi, "increment"),)
        # )

        # print(
        #    "Mint: ",
        (self.mint_erc20(self.egm_contract_address, self.egm_abi, self.acc.address, 100),)
        # )

        print("OK")
