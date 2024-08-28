import time

import flexitest

WAIT_TIME = 0.2


@flexitest.register
class BridgeMsgTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()

        # BridgeMessage { source_id: 481563127,
        #                 sig: 747b078c509658461e64699abcf2dc....,
        #                 scope: [197], payload: [] }
        blobdata = "".join(
            [
                "00000000d2cff500e063f9f4b4d8bd842eae6f9b17833e997779bab28397ab72",
                "2f1fdead39e8a378f8c15328076ae1cb45ce26aeb07bf6864d52f924658b141a",
                "a72425d4000a0000000700000001020304050607aa44af3c",
            ]
        )

        seqrpc.alpbridgemsg_submitRawMsg(blobdata)

        time.sleep(WAIT_TIME)

        # VODepositSig(10)
        scope = "000a000000"
        print(scope)

        msg = seqrpc.alpbridgemsg_getMsgsByScope(scope)
        print(msg)

        # check if received blobdata and sent blobdata are same or not
        assert blobdata == msg[0]
