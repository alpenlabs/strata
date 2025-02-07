from enum import Enum
from typing import TypeAlias

import solcx
import web3
from eth_typing import HexStr
from hexbytes import HexBytes
from web3.types import TxReceipt

from .account import AbstractAccount

solcx.install_solc("0.8.0")


class TransactionType(Enum):
    LEGACY = 1
    EIP2930 = 2
    EIP1559 = 3


Tx: TypeAlias = dict[str, int | str]


class _TransactionFaucet:
    """
    Base class for all transaction mixins that equips those with account and w3.
    """

    def __init__(self, acc: AbstractAccount):
        self._acc = acc

    @property
    def w3(self) -> web3.Web3:
        return self._acc.w3


class TransactionBuilder(_TransactionFaucet):
    """
    A transaction mixin responsible for basic trancation payload building.
    Supports Legacy, EIP-2930 and EIP-1559 transaction types.
    """

    def ensure_tx(
        self, tx: Tx, tx_type: TransactionType = TransactionType.LEGACY, from_rpc: bool = False
    ):
        tx.setdefault("from", self._acc.address)
        tx.setdefault("nonce", self._acc.nonce)

        if tx_type == TransactionType.LEGACY:
            tx.setdefault(
                "gasPrice", self.w3.eth.gas_price if not from_rpc else self.w3.to_wei("1", "gwei")
            )
        elif tx_type == TransactionType.EIP2930:
            tx.setdefault("type", "0x1")
            tx.setdefault("chainId", self.w3.eth.chain_id)
            # Define an empty access_list for simplicity for now.
            tx.setdefault("accessList", [{"address": tx["to"], "storageKeys": []}])

            tx.setdefault(
                "gasPrice", self.w3.eth.gas_price if from_rpc else self.w3.to_wei("1", "gwei")
            )

        elif tx_type == TransactionType.EIP1559:
            tx.setdefault("type", "0x2")
            tx.setdefault("chainId", self.w3.eth.chain_id)

            # TODO: use from_rpc to fetch the current fee market if needed.
            # Currently hardcoded.
            tx.setdefault("maxPriorityFeePerGas", self.w3.to_wei("1", "gwei"))
            tx.setdefault("maxFeePerGas", self.w3.to_wei("2", "gwei"))

    @classmethod
    def new_with_gas(cls, gas: int) -> Tx:
        return {"gas": gas}


class TransactionSender(TransactionBuilder):
    """
    A helper that's capable of sending transaction and waiting for the receipt.
    """

    def send_tx(self, tx: Tx) -> HexStr | None:
        try:
            tx_hash = self.w3.eth.send_transaction(tx)
            return self.w3.to_hex(tx_hash)
        except Exception as e:
            print(e)
            return None

    def send_tx_and_wait(self, tx: Tx, timeout: int = 10) -> TxReceipt | None:
        try:
            tx_hash = self.w3.eth.send_transaction(tx)
            return self.w3.eth.wait_for_transaction_receipt(tx_hash, timeout=timeout)
        except Exception as e:
            print(e)
            return None

    def send_ensured_tx(self, tx: Tx, tx_type: TransactionType) -> HexStr | None:
        self.ensure_tx(tx, tx_type)
        return self.send_tx(tx)

    def send_ensured_tx_and_wait(
        self, tx: Tx, tx_type: TransactionType, timeout: int = 10
    ) -> TxReceipt | None:
        self.ensure_tx(tx, tx_type)
        return self.send_tx_and_wait(tx, timeout=timeout)

    def wait_tx(self, tx_hash: HexBytes, timeout: int = 10) -> TxReceipt | None:
        try:
            return self.w3.eth.wait_for_transaction_receipt(tx_hash, timeout=timeout)
        except Exception as e:
            print(e)
            return None


class TransferTransaction(TransactionSender):
    """
    A transaction mixin that's capable of transferring funds to another account.
    """

    def transfer(self, to, value, tx_type: TransactionType) -> HexStr | None:
        tx: Tx = TransactionBuilder.new_with_gas(25000)
        tx.update(
            {
                "to": to,
                "value": self.w3.to_wei(value, "ether"),
            }
        )
        return self.send_ensured_tx(tx, tx_type)


class SmartContracts(TransactionSender):
    """
    Mixin to work with smart contracts.
    """

    CONTRACTS_DIR = "load/reth/contracts/"
    SOL_VERSION = "0.8.0"

    _smart_contracts_storage = dict()

    def _extract_compiled_contract(self, compiled_sol, contract_name):
        for ct_id, ct_interface in compiled_sol.items():
            if ct_id.endswith(contract_name):
                return ct_interface["abi"], ct_interface["bin"]
        return None

    def _compile_contract(self, filename, contract_name=None):
        if contract_name is None:
            contract_name = filename.split(".")[0]

        compiled_sol = solcx.compile_files(
            [f"{SmartContracts.CONTRACTS_DIR}{filename}"],
            output_values=["abi", "bin"],
            solc_version=SmartContracts.SOL_VERSION,
        )
        return self._extract_compiled_contract(compiled_sol, contract_name)

    def deploy_contract(
        self,
        contract_path,
        contract_name,
        contract_id,
        *args,
    ):
        abi, bytecode = self._compile_contract(contract_path, contract_name)
        contract = self.w3.eth.contract(abi=abi, bytecode=bytecode)

        tx: Tx = TransactionBuilder.new_with_gas(5_000_000)
        self.ensure_tx(tx)

        tx = contract.constructor(*args).build_transaction(tx)
        receipt: TxReceipt = self.send_tx_and_wait(tx)
        self._smart_contracts_storage[contract_id] = (receipt.contractAddress, abi)
        return receipt.contractAddress, abi

    def _contract_abi(self, contract_id):
        return self._smart_contracts_storage.get(contract_id, (None, None))[1]

    def _contract_address(self, contract_id):
        return self._smart_contracts_storage.get(contract_id, (None, None))[0]

    def call_contract(self, contract_id, function_name, *args) -> HexBytes | None:
        try:
            contract = self.w3.eth.contract(
                address=self._contract_address(contract_id), abi=self._contract_abi(contract_id)
            )
            tx: Tx = TransactionBuilder.new_with_gas(1_000_000)
            self.ensure_tx(tx)

            function_tx = contract.functions[function_name](*args).build_transaction(tx)
            return self.send_tx(function_tx)
        except Exception as e:
            print(e)
            return None

    def get_contract_address(self, contract_id):
        return self.w3.to_checksum_address(self._contract_address(contract_id))


class ERC20(SmartContracts):
    """
    Allows to call ERC20 contract methods.
    """

    def mint_erc20(self, token_name, amount):
        return self.call_contract(token_name, "mint", self._acc.address, amount)

    def approve_spend(self, token_name, spender, amount):
        return self.call_contract(token_name, "approve", spender, amount)


class Uniswap(SmartContracts):
    """
    Allows to call basic uniswap methods.
    """

    def swap(self, token_in, token_out, amount):
        return self.call_contract("Uniswap", "swap", token_in, token_out, amount)

    def add_liquidity(self, token_a, amount_a, token_b, amount_b):
        return self.call_contract(
            "Uniswap",
            "addLiquidity",
            token_a,
            token_b,
            amount_a,
            amount_b,
        )


class EthTransactions(
    Uniswap, ERC20, SmartContracts, TransferTransaction, TransactionSender, _TransactionFaucet
):
    """
    Enables all mixins altogether.
    """

    pass
