import random

from locust import task

from .reth import BaseRethLoadJob
from .transaction import EthTransactions, TransactionType, TransferTransaction


class BasicRethBlockJob(BaseRethLoadJob):
    """
    Basic job that displays block information - number of transactions.
    """

    def before_start(self):
        super().before_start()
        self._acc = self.new_account()
        self._block_number = 0

    @task
    def get_block(self):
        block_number = self._acc.w3.eth.get_block_number()
        for i in range(self._block_number + 1, block_number + 1):
            try:
                block = self._acc.w3.eth.get_block(hex(i))
            except Exception:
                break
            x = block["hash"].hex()
            self._logger.info(
                f"NEW BLOCK: num={i} => tx_count={len(block['transactions'])}, gas={block['gasUsed']}, hash={x}"
            )
            self._block_number = i


class User:
    def __init__(self, tx, transfer):
        self.tx = tx
        self.transfer = transfer

    @classmethod
    def from_acc(cls, job):
        acc = job.new_account()
        logger = job._logger
        return User(EthTransactions(acc, logger), TransferTransaction(acc, logger))


class BasicRethTxJob(BaseRethLoadJob):
    """
    Basic job that generates the load - transfers, ERC20 mints, smart contract calls.
    """

    all_users = list()

    def before_start(self):
        super().before_start()

        for _ in range(10):
            self.all_users.append(User.from_acc(self))

    @property
    def tx(self):
        return random.choice(self.all_users).tx

    @property
    def transfer(self):
        return random.choice(self.all_users).transfer

    def after_start(self):
        super().after_start()

        # Deploy Counter.
        self.tx.deploy_contract("Counter.sol", "Counter", "Counter")

        # Deploy EGM and SUSD
        self.tx.deploy_contract("ERC20.sol", "ERC20", "EGM", "EndGameMoney", "EGM")
        self.tx.deploy_contract("ERC20.sol", "ERC20", "SUSD", "StrataUSD", "SUSDD")

        # Deploy Uniswap.
        self.tx.deploy_contract("Uniswap.sol", "UniswapFactory", "UniswapFactory")
        self.tx.deploy_contract(
            "Uniswap.sol",
            "UniswapRouter",
            "Uniswap",
            self.tx.get_contract_address("UniswapFactory"),
        )

        _tx = self.tx
        # Mint some EGM and SUSD tokens.
        _tx.mint_erc20("EGM", 1_000_000_000)
        _tx.mint_erc20("SUSD", 1_000_000_000)

        uniswap_addr = _tx.get_contract_address("Uniswap")
        egm_token_addr = _tx.get_contract_address("EGM")
        susd_token_addr = _tx.get_contract_address("SUSD")

        # Approve spending tokens to Uniswap (standard ERC20 approve).
        _tx.approve_spend("EGM", uniswap_addr, 1_000_000_000)
        _tx.approve_spend("SUSD", uniswap_addr, 1_000_000_000)

        # Add liquidity to uniswap liquidity pair (since we approved spending).
        _tx.add_liquidity(egm_token_addr, 100_000_000, susd_token_addr, 100_000_000)

    @task
    def transactions_task(self):
        for _ in range(3):
            self.send(self.transfer)

        # Increment Counter.

        self.tx.call_contract("Counter", "increment", wait=False)

        _tx = self.tx
        egm_token_addr = _tx.get_contract_address("EGM")
        susd_token_addr = _tx.get_contract_address("SUSD")

        # Mint some SUSD.
        self.tx.mint_erc20("SUSD", 100, wait=False)
        self.tx.swap(susd_token_addr, egm_token_addr, 500, wait=False)

        self._logger.info("task completed successfully.")

    def send(self, tr):
        target_address = self.tx.w3.eth.account.create().address

        tr.transfer(
            target_address,
            0.1,
            random.choice(
                [TransactionType.LEGACY, TransactionType.EIP1559, TransactionType.EIP2930]
            ),
        )
