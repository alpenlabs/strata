import threading
import time

from bitcoinlib.services.bitcoind import BitcoindClient

block_list = []


def generate_task(rpc: BitcoindClient, block_count, wait_dur, addr, infinite):
    print("generating to address", addr)

    def gen_to_addr():
        time.sleep(wait_dur)
        try:
            blk = rpc.proxy.generatetoaddress(1, addr)
            block_list.append(blk[0])
            print("made block", blk)
        except:
            return

    if infinite:
        while True:
            gen_to_addr()
    else:
        for _ in range(0, block_count):
            gen_to_addr()


def create_wallet(bitcoin_rpc: BitcoindClient):
    bitcoin_rpc.proxy.createwallet("dummy")


def generate_blocks(
    bitcoin_rpc: BitcoindClient, wait_dur, block_count=10, infinite=False
):
    addr = bitcoin_rpc.proxy.getnewaddress()
    thr = threading.Thread(
        target=generate_task, args=(bitcoin_rpc, block_count, wait_dur, addr, infinite)
    )
    thr.start()
