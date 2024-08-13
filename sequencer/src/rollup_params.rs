use alpen_express_primitives::{block_credential::CredRule, params::RollupParams};
use format_serde_error::SerdeError;
use serde::Deserialize;

use crate::InitError;

pub(crate) fn parse_rollup_params_json(json: String) -> Result<RollupParams, InitError> {
    let json_params = serde_json::from_str::<JsonRollupParams>(&json)
        .map_err(|err| SerdeError::new(json.to_string(), err))?;

    Ok(json_params.into())
}

#[derive(Debug, Deserialize)]
struct JsonRollupParams {
    pub rollup_name: String,
    pub block_time: u64,
    pub cred_rule: JsonCredRule,
    pub horizon_l1_height: u64,
    pub genesis_l1_height: u64,
    #[serde(with = "hex::serde")]
    pub evm_genesis_block_hash: [u8; 32],
    #[serde(with = "hex::serde")]
    pub evm_genesis_block_state_root: [u8; 32],
    pub l1_reorg_safe_depth: u32,
    pub batch_l2_blocks_target: u64,
}

impl From<JsonRollupParams> for RollupParams {
    fn from(value: JsonRollupParams) -> Self {
        Self {
            rollup_name: value.rollup_name,
            block_time: value.block_time,
            cred_rule: value.cred_rule.into(),
            horizon_l1_height: value.horizon_l1_height,
            genesis_l1_height: value.genesis_l1_height,
            evm_genesis_block_hash: value.evm_genesis_block_hash.into(),
            evm_genesis_block_state_root: value.evm_genesis_block_state_root.into(),
            l1_reorg_safe_depth: value.l1_reorg_safe_depth,
            batch_l2_blocks_target: value.batch_l2_blocks_target,
        }
    }
}

#[derive(Debug, Deserialize)]
enum JsonCredRule {
    Unchecked,
    #[serde(with = "hex::serde")]
    SchnorrKey([u8; 32]),
}

impl From<JsonCredRule> for CredRule {
    fn from(value: JsonCredRule) -> Self {
        match value {
            JsonCredRule::Unchecked => CredRule::Unchecked,
            JsonCredRule::SchnorrKey(key) => CredRule::SchnorrKey(key.into()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn deserialize_rollup_config() {
        let json = r#"
        {
            "rollup_name": "test",
            "block_time": 5000,
            "cred_rule": "Unchecked",
            "horizon_l1_height": 5,
            "genesis_l1_height": 10,
            "evm_genesis_block_hash": "0000000000000000000000000000000000000000000000000000000000000000",
            "evm_genesis_block_state_root": "0101010101010101010101010101010101010101010101010101010101010101",
            "l1_reorg_safe_depth": 5,
            "batch_l2_blocks_target": 100
        }
        "#;

        let params = parse_rollup_params_json(json.to_string()).unwrap();

        assert_eq!(
            params,
            JsonRollupParams {
                rollup_name: "test".into(),
                block_time: 5000,
                cred_rule: JsonCredRule::Unchecked,
                horizon_l1_height: 5,
                genesis_l1_height: 10,
                evm_genesis_block_hash: [0; 32],
                evm_genesis_block_state_root: [1; 32],
                l1_reorg_safe_depth: 5,
                batch_l2_blocks_target: 100,
            }
            .into()
        );
    }
}
