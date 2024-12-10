from pydantic import BaseModel


class CredRule(BaseModel):
    schnorr_key: str


class OperatorConfigItem(BaseModel):
    signing_pk: str
    wallet_pk: str


class OperatorConfig(BaseModel):
    static: list[OperatorConfigItem]

    def get_operators_pubkeys(self) -> list[str]:
        return [operator.wallet_pk for operator in self.static]


class RollupVk(BaseModel):
    sp1: str


class ProofPublishMode(BaseModel):
    timeout: int


class RollupConfig(BaseModel):
    """
    A rollup params config data-class.
    Can be used to work with config values conveniently.
    """

    rollup_name: str
    block_time: int
    cred_rule: CredRule
    horizon_l1_height: int
    genesis_l1_height: int
    operator_config: OperatorConfig
    evm_genesis_block_hash: str
    evm_genesis_block_state_root: str
    l1_reorg_safe_depth: int
    target_l2_batch_size: int
    address_length: int
    deposit_amount: int
    rollup_vk: RollupVk
    dispatch_assignment_dur: int
    proof_publish_mode: ProofPublishMode
    max_deposits_in_block: int
    network: str
