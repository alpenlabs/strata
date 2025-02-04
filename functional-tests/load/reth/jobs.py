from locust import task

from .reth import BaseRethLoadJob
from .transaction import EthTransactions, TransactionType, TransferTransaction


class BasicRethBlockJob(BaseRethLoadJob):
    def before_start(self):
        super().before_start()
        self._acc = self.new_account()

    @task
    def get_block(self):
        # print("GET_BLOCK REQUEST")

        block_txn_cnt = list()
        for i in range(1, 50):
            try:
                block = self._acc.w3.eth.get_block(hex(i))
            except Exception:
                break
            txn_cnt = len(block["transactions"])
            block_txn_cnt.append(txn_cnt)
        # print(f"BLOCKS {block_txn_cnt}")


class BasicRethTxJob(BaseRethLoadJob):
    def before_start(self):
        super().before_start()

        self._acc = self.new_account()

        self.eth = EthTransactions(self._acc)
        self.transfer = TransferTransaction(self._acc)

    def after_start(self):
        super().after_start()

        print(
            "Deploying counter contract: ",
            self.eth.deploy_contract("Counter.sol", "Counter", "Counter"),
        )

        print(
            "Deploying EGM contract: ",
            self.eth.deploy_contract("ERC20.sol", "ERC20", "EGM", "EndGameMoney", "EGM"),
        )

        print(
            "Deploying SUSD token: ",
            self.eth.deploy_contract("ERC20.sol", "ERC20", "SUSD", "StrataUSD", "SUSD"),
        )

        print(
            "Deploying Uniswap Factory: ",
            self.eth.deploy_contract("Uniswap.sol", "UniswapFactory", "UniswapFactory"),
        )

        print(
            "Deploying Uniswap: ",
            self.eth.deploy_contract(
                "Uniswap.sol",
                "UniswapRouter",
                "Uniswap",
                self.eth.get_contract_address("UniswapFactory"),
            ),
        )

        print("Mint EGM: ", self.eth.mint_erc20("EGM", 1_000_000))
        print("Mint SUSD: ", self.eth.mint_erc20("SUSD", 1_000_000))

        uniswap_addr = self.eth.get_contract_address("Uniswap")
        egm_token_addr = self.eth.get_contract_address("EGM")
        susd_token_addr = self.eth.get_contract_address("SUSD")

        print(
            "Approve EGM: ",
            self.eth.approve_spend("EGM", uniswap_addr, 1_000_000),
        )

        print(
            "Approve SUSD: ",
            self.eth.approve_spend("SUSD", uniswap_addr, 1_000_000),
        )

        print(
            "Add liquidity: ",
            self.eth.add_liquidity(
                egm_token_addr,
                100_000,
                susd_token_addr,
                100_000,
            ),
        )

        return
        # We either have a bug in our reth, or swap itself contains some neat bug.
        # Disabled for now.
        # TODO: investigate.
        print(
            "Swap tokens: ",
            self.eth.swap(
                egm_token_addr,
                susd_token_addr,
                5_000,
            ),
        )

    @task
    def transactions_task(self):
        target_address = self.eth.w3.eth.account.create().address

        print(
            "Sending Legacy Transaction:",
            self.transfer.transfer(target_address, 0.1, TransactionType.LEGACY),
        )

        print(
            "Sending 2930 Transaction:",
            self.transfer.transfer(target_address, 0.1, TransactionType.EIP2930),
        )

        print(
            "Sending 1559 Transaction:",
            self.transfer.transfer(target_address, 0.1, TransactionType.EIP1559),
        )

        print(
            "Incrementing Counter:",
            self.eth.call_contract("Counter", "increment"),
        )

        print(
            "Mint: ",
            self.eth.mint_erc20("SUSD", 100),
        )

        print("OK")
