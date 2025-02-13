from locust import task

from .reth import BaseRethLoadJob
from .transaction import EthTransactions, TransactionType


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
            self._logger.info(f"NEW BLOCK: num={i} => tx_count={len(block['transactions'])}")
            self._block_number = i


class BasicRethTxJob(BaseRethLoadJob):
    """
    Basic job that generates the load - transfers, ERC20 mints, smart contract calls.
    """

    def before_start(self):
        super().before_start()

        self._acc = self.new_account()
        self.tx = EthTransactions(self._acc, self._logger)

    def after_start(self):
        super().after_start()

    @task
    def transactions_task(self):
        target_address = self.tx.w3.eth.account.create().address
        try:
            self.tx.transfer(target_address, 0.1, TransactionType.EIP2930, wait=True)
        except Exception as e:
            print(e)
        self._logger.info("task completed successfully.")
