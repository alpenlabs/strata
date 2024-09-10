import time

import flexitest


@flexitest.register
class ProverClientTest(flexitest.Test):
    def __init__(self, ctx: flexitest.InitContext):
        ctx.set_env("prover")

    def main(self, ctx: flexitest.RunContext):
        prover_client = ctx.get_service("prover_client")
        prover_client_rpc = prover_client.create_rpc()

        time.sleep(1)
        rpc_res = prover_client_rpc.dev_alp_prove_el_block(10)
        print("got the rpc res: {}",rpc_res)

