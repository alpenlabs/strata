import web3
from gevent.lock import Semaphore
from web3.middleware.signing import SignAndSendRawMiddlewareBuilder

from load.job import StrataLoadJob


class AbstractAccount:
    _nonce: int = 0
    _nonce_lock = Semaphore()

    @property
    def w3(self) -> web3.Web3:
        raise NotImplementedError("w3 should be implemented by subclasses")

    @property
    def account(self):
        raise NotImplementedError("account should be implemented by subclasses")

    @property
    def nonce(self):
        with self._nonce_lock:
            nonce = self._nonce
            self._nonce += 1
            return nonce

    @property
    def address(self):
        return self.account.address

    @property
    def balance(self):
        return self.w3.eth.account.get_balance(self.address)


class GenesisAccount:
    nonce: int = 0
    nonce_lock = Semaphore()

    def __init__(self, job: StrataLoadJob):
        w3 = web3.Web3(web3.Web3.HTTPProvider(job.host, session=job.client))
        # Init the prefunded account as specified in the chain config.
        account = w3.eth.account.from_key(
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
        )
        # Set the account onto web3 and init the signing middleware.
        w3.address = account.address
        w3.middleware_onion.add(SignAndSendRawMiddlewareBuilder.build(account))
        self._w3 = w3
        self._account = account

    def fund_address(self, account_address, amount) -> bool:
        # Class Descriptor attribute to have the same nonce lock even if
        # multiple instances of GenesisAccount are used.
        nonce = GenesisAccount._inc_nonce()
        tx_hash = self._w3.eth.send_transaction(
            {
                "to": account_address,
                "value": hex(amount),
                "gas": hex(100000),
                "from": self._account.address,
                "nonce": nonce,
            }
        )

        # Block on this transaction to make sure funding is successful before proceeding further.
        tx_receipt = self._w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
        return tx_receipt["status"] == 1

    @classmethod
    def _inc_nonce(cls):
        with cls.nonce_lock:
            nonce = cls.nonce
            cls.nonce += 1
            return nonce


class FundedAccount(AbstractAccount):
    def __init__(self, job: StrataLoadJob):
        w3 = web3.Web3(web3.Web3.HTTPProvider(job.host, session=job.client))
        # Init the prefunded account as specified in the chain config.
        account = w3.eth.account.create()
        # Set the account onto web3 and init the signing middleware.
        w3.address = account.address
        w3.middleware_onion.add(SignAndSendRawMiddlewareBuilder.build(account))
        self._w3 = w3
        self._account = account

    def fund_me(self, genesis_acc: GenesisAccount, amount=1_000_000_000_000_000_000_000):
        genesis_acc.fund_address(self.address, amount)

    @property
    def w3(self):
        return self._w3

    @property
    def account(self):
        return self._account
