import time

import flexitest

WAIT_TIME = 2


@flexitest.register
class BridgeMsgTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("basic")

    def main(self, ctx: flexitest.RunContext):
        seq = ctx.get_service("sequencer")

        # create both btc and sequencer RPC
        seqrpc = seq.create_rpc()

        # BridgeMessage { source_id: 1,
        #                 sig: [00] * 64
        #                 scope: Misc, payload: [42] }
        raw_msg = "".join(
            [
                "01000000",
                "00" * 64,
                "01000000" + "00",
                "01000000" + "42",
            ]
        )

        seqrpc.alp_submitBridgeMsg(raw_msg)

        time.sleep(WAIT_TIME + 2)

        # VODepositSig(10)
        scope = "00"
        print(scope)

        msgs = seqrpc.alp_getBridgeMsgsByScope(scope)
        print(msgs)

        # check if received blobdata and sent blobdata are same or not
        assert len(msgs) == 1, "wrong number of messages in response"
        assert msgs[0] == raw_msg, "not the message we expected"
