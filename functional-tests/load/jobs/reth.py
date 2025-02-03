from locust import task

from load.job import BaseRethLoadJob


class EthJob(BaseRethLoadJob):
    def on_start(self):
        super().on_start()

        w3, new_acc = self.w3_with_new_acc()
        self.w3 = w3
        self.new_acc = new_acc

        b = self._balance(new_acc.address)
        print(f"BALANCE AFTER START: {b}")

    @task
    def get_block(self):
        print("GET_BLOCK REQUEST")
        for i in range(1, 10):
            block = self.w3.eth.get_block(hex(i))
            num = block["number"]
            txn_cnt = len(block["transactions"])
            hash = block["hash"]
            print(f"BLOCK DATA \t\t\t\t\t {hash}, {num}, {txn_cnt}")

    @task
    def block_num(self):
        print("BLOCK_NUM REQUEST")
        # Pure json-rpc without web3 with middleware.
        method = "eth_blockNumber"
        params = []
        payload = {"jsonrpc": "2.0", "method": method, "params": params, "id": 1}
        headers = {"Content-type": "application/json"}
        # response = session.post(self.host, json=payload, headers=headers)
        response = self.client.post("", json=payload, headers=headers)
        # print(f"raw json response: {response.json()}")
        print("BLOCK_NUMBER: {}".format(response.json()["result"]))

    @task(5)
    def send(self):
        print("TRANSFER TRANSACTION")

        source = self.w3.address
        dest = self.w3.to_checksum_address("0x0000000000000000000000000000000000000001")
        to_transfer = 1_000_000_000_000_000_000
        try:
            tx_hash = self.w3.eth.send_transaction(
                {"to": dest, "value": hex(to_transfer), "gas": hex(100000), "from": source}
            )
            print(f"transfer transaction hash: {tx_hash}")
        except Exception as e:
            print(e)
