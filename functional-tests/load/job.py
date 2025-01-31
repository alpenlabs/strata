import web3
import web3.middleware
from locust import HttpUser


class StrataLoadJob(HttpUser):
    """
    A common layer for all the load jobs in the load tests.
    """

    pass


# TODO(load): configure the structured logging as we do in the tests.
class BaseRethLoadJob(StrataLoadJob):
    fund_amount: int = 1_000_000_000_000_000_000_000  # 1000 ETH

    def on_start(self):
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
        w3.middleware_onion.add(web3.middleware.SignAndSendRawMiddlewareBuilder.build(account))

        return w3, account

    def _fund_account(self, acc):
        print(f"FUNDING ACCOUNT {acc}")
        source = self._root_w3.address
        tx_hash = self._root_w3.eth.send_transaction(
            {"to": acc, "value": hex(self.fund_amount), "gas": hex(100000), "from": source}
        )

        tx_receipt = self._root_w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
        print(f"FUNDING SUCCESS: {tx_receipt}")

    def _balance(self, acc):
        return self._root_w3.eth.get_balance(acc)
