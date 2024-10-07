import time

import flexitest


@flexitest.register
class ProverClientTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        # Wait for the some block building
        print("Test started just sleeping...")
        time.sleep(60)
        print("Sleep 2")

        # use_latest_ckp = False
        # if use_latest_ckp:
        #     checkpoint_idx = 1
        #     l1_range = (1, 25)
        #     l2_range = (1, 25)
        #     rpc_res = prover_client_rpc.dev_strata_proveCheckpointRaw(
        #         checkpoint_idx, l1_range, l2_range
        #     )
        #     print("got the rpc res: {}", rpc_res)
        #     assert rpc_res is not None
        # else:
        #     rpc_res = prover_client_rpc.dev_strata_proveLatestCheckPoint()
        #     print("got the rpc res: {}", rpc_res)
        #     assert rpc_res is not None

        time.sleep(60 * 10)
