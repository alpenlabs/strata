import flexitest

from envs import net_settings, testenv
from mixins.bridge_out_precompile_contract_mixin import BridgePrecompileMixin


@flexitest.register
class ContractBridgeOutWithNoValueTest(BridgePrecompileMixin):
    def __init__(self, ctx: flexitest.InitContext):
        fast_batch_settings = net_settings.get_fast_batch_settings()
        ctx.set_env(
            testenv.BasicEnvConfig(pre_generate_blocks=101, rollup_settings=fast_batch_settings)
        )

    def main(self, ctx: flexitest.RunContext):
        # no need to deposit as we are just calling the contract with no value

        # Call the contract function
        contract_instance = self.web3.eth.contract(
            abi=self.abi, address=self.deployed_contract_receipt.contractAddress
        )
        tx_hash = contract_instance.functions.withdrawWithoutBalance(self.bosd).transact(
            {"gas": 5_000_000}
        )

        tx_receipt = self.web3.eth.wait_for_transaction_receipt(tx_hash, timeout=30)
        # should fail because we are calling without values
        assert tx_receipt.status == 0
