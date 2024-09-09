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
                "0000000069b125a60f343e833805e9c55bdd42953b6803ac0e262145",
                "005dd038e69c91a73d7773ecf4f236fa94b9356c41ab4993e5cd51d1",
                "ab744f3b4258d47f26e03f01000a0000000700000001020304050607",
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
