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

    # Additional fields that aren't coming from datatool config generation (yet)
    # and has to be supplied manually.
    # TODO: extend datatool to return OPERATOR_FEE from bridge-tx-builder/src/constants.rs
    operator_fee: int = 50_000_000
    # TODO: this is currently an inconsistent mess, figure it out.
    # ANYONE_CAN_SPEND_OUTPUT_VALUE (330) in `bridge-tx-builder/src/constants.rs`
    # + 5.5 sats/vB (200 vbytes) according to `MIN_RELAY_FEE`
    # in `bridge-tx-builder/src/constants.rs`
    withdraw_extra_fee: int = int(330 + 5.5 * 200)
