from locust import task

from .reth import BaseRethLoadJob
from .transaction import EthTransactions, TransactionType, TransferTransaction


class BasicRethBlockJob(BaseRethLoadJob):
    """
    Basic job thats displays block information - number of transactions.
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
            self._logger.info(f"NEW BLOCK: num={i} => tx_count={len(block['transactions'])}")
            self._block_number = i


class BasicRethTxJob(BaseRethLoadJob):
    """
    Basic job thats generates the load - transfers, ERC20 mints, smart contract calls.
    """

    def before_start(self):
        super().before_start()

        self._acc = self.new_account()

        self.tx = EthTransactions(self._acc, self._logger)
        self.transfer = TransferTransaction(self._acc, self._logger)

    def after_start(self):
        super().after_start()

        # Deploy Counter.
        self.tx.deploy_contract("Counter.sol", "Counter", "Counter")

        # Deploy EGM and SUSD
        self.tx.deploy_contract("ERC20.sol", "ERC20", "EGM", "EndGameMoney", "EGM")
        self.tx.deploy_contract("ERC20.sol", "ERC20", "SUSD", "StrataUSD", "SUSD")

        # Deploy Uniswap.
        self.tx.deploy_contract("Uniswap.sol", "UniswapFactory", "UniswapFactory")
        self.tx.deploy_contract(
            "Uniswap.sol",
            "UniswapRouter",
            "Uniswap",
            self.tx.get_contract_address("UniswapFactory"),
        )

        # Mint some EGM and SUSD tokens.
        self.tx.mint_erc20("EGM", 1_000_000)
        self.tx.mint_erc20("SUSD", 1_000_000)

        uniswap_addr = self.tx.get_contract_address("Uniswap")
        egm_token_addr = self.tx.get_contract_address("EGM")
        susd_token_addr = self.tx.get_contract_address("SUSD")

        # Approve spending tokens to Uniswap (standard ERC20 approve).
        self.tx.approve_spend("EGM", uniswap_addr, 1_000_000)
        self.tx.approve_spend("SUSD", uniswap_addr, 1_000_000)

        # Add liquidity to uniswap liquidity pair (since we approved spending).
        self.tx.add_liquidity(
            egm_token_addr,
            100_000,
            susd_token_addr,
            100_000,
        )

        # We either have a bug in our reth, or swap itself contains some neat bug.
        # Disabled for now.
        # TODO: investigate.
        return

        # Swap SUSD to EGM (FOMO IS REAL).
        self.tx.swap(susd_token_addr, egm_token_addr, 500_000)

    @task
    def transactions_task(self):
        target_address = self.tx.w3.eth.account.create().address

        # Couple of transfers with different tx types.
        self.transfer.transfer(target_address, 0.1, TransactionType.LEGACY)
        self.transfer.transfer(target_address, 0.1, TransactionType.EIP2930)
        self.transfer.transfer(target_address, 0.1, TransactionType.EIP1559)

        # Increment Counter.
        self.tx.call_contract("Counter", "increment")

        # Mint some SUSD.
        self.tx.mint_erc20("SUSD", 100)

        self._logger.info("task completed successfully.")
