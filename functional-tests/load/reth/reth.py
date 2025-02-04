from enum import Enum

import web3
from gevent.lock import Semaphore
from web3.middleware.signing import SignAndSendRawMiddlewareBuilder

from load.job import StrataLoadJob


class TransactionType(Enum):
    LEGACY = 1
    EIP2930 = 2
    EIP1559 = 3


# TODO(load): configure the structured logging as we do in the tests.
class BaseRethLoadJob(StrataLoadJob):
    fund_amount: int = 1_000_000_000_000_000_000_000
    """
    The funding amount a new account is granted with. Default is 1000 ETH
    """

    root_acc_nonce: int = 0
    root_acc_nonce_lock = Semaphore()
    """
    The nonce of the genesis account.
    
    P.S. It's needed to maintain it manually because of two reasons:
    1. To eliminate an eth_getTransactionCount RPC and improve the performance
    2. To specify exact nonce, so transactions are properly chained.

    Also, usage of the gevent syncronization primitive is essential here, otherwise the
    funding transactions from different load jobs (running in different green threads)
    may spawn funding transactions with the same nonce and one of them is replaced by the other.
    """

    def before_start(self):
        super().before_start()
        root_w3, genesis_acc = self.w3_with_genesis_acc()
        self._root_w3 = root_w3
        self._genesis_acc = genesis_acc

    def w3_with_genesis_acc(self):
        """
        Return w3 with prefunded "root" account as specified in the chain config.
        """
        return self._init_w3(
            lambda w3: w3.eth.account.from_key(
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
            )
        )

    def w3_with_new_acc(self):
        """
        Return w3 with a fresh account.
        Also, funds this account, so it's able to sign and send some txns.
        """
        w3, new_acc = self._init_w3(lambda w3: w3.eth.account.create())
        self._fund_account(new_acc.address)

        return w3, new_acc

    def _init_w3(self, init):
        # Reuse the http session by locust internals, so the stats are measured correctly.
        w3 = web3.Web3(web3.Web3.HTTPProvider(self.host, session=self.client))
        # Init the account according to lambda
        account = init(w3)
        # Set the account onto web3 and init the signing middleware.
        w3.address = account.address
        w3.middleware_onion.add(SignAndSendRawMiddlewareBuilder.build(account))

        return w3, account

    @classmethod
    def _inc_root_acc_nonce(cls):
        with cls.root_acc_nonce_lock:
            nonce = cls.root_acc_nonce
            cls.root_acc_nonce += 1
            return nonce

    def _fund_account(self, acc):
        nonce = BaseRethLoadJob._inc_root_acc_nonce()

        print(f"FUNDING ACCOUNT {acc}")
        tx_hash = self._root_w3.eth.send_transaction(
            {
                "to": acc,
                "value": hex(self.fund_amount),
                "gas": hex(100000),
                "from": self._root_w3.address,
                "nonce": nonce,
            }
        )

        # Block on this transaction to make sure funding is successful before proceeding further.
        tx_receipt = self._root_w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
        print(f"FUNDING SUCCESS: {tx_receipt}")

    def _balance(self, acc):
        return self._root_w3.eth.get_balance(acc)
