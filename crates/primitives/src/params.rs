//! Global consensus parameters for the rollup.

use serde::{Deserialize, Serialize};

use crate::{
    block_credential::CredRule, operator::OperatorPubkeys, prelude::Buf32, vk::RollupVerifyingKey,
};

/// Consensus parameters that don't change for the lifetime of the network
/// (unless there's some weird hard fork).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RollupParams {
    /// Rollup name
    pub rollup_name: String,

    /// Block time in milliseconds.
    pub block_time: u64,

    /// Rule we use to decide if a block is correctly signed.
    pub cred_rule: CredRule,

    /// Block height from which to watch for L1 transactions
    pub horizon_l1_height: u64,

    /// Block height we'll construct the L2 genesis block from.
    pub genesis_l1_height: u64,

    /// Config for how the genesis operator table is set up.
    pub operator_config: OperatorConfig,

    /// Hardcoded EL genesis info
    /// TODO: move elsewhere
    pub evm_genesis_block_hash: Buf32,
    pub evm_genesis_block_state_root: Buf32,

    /// Depth after which we consider the L1 block to not reorg
    pub l1_reorg_safe_depth: u32,

    /// target batch size in number of l2 blocks
    pub target_l2_batch_size: u64,

    /// Maximum length of an EE address in a deposit.
    // FIXME this should be "max address length"
    pub address_length: u8,

    /// Exact "at-rest" deposit amount, in sats.
    pub deposit_amount: u64,

    /// SP1 verifying key that is used to verify the Groth16 proof posted on Bitcoin
    // FIXME which proof?  should this be `checkpoint_vk`?
    pub rollup_vk: RollupVerifyingKey,

    /// Whether to verify the proofs from L1 or not.
    pub verify_proofs: bool,

    /// Number of Bitcoin blocks a withdrawal dispatch assignment is valid for.
    pub dispatch_assignment_dur: u32,

    /// Describes how proofs are published
    pub proof_publish_mode: ProofPublishMode,

    /// max number of deposits in a block
    pub max_deposits_in_block: u8,
}

/// Configuration common among deposit and deposit request transaction
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DepositTxParams {
    /// magic bytes, usually a rollup name
    pub magic_bytes: Vec<u8>,
    /// EE Address length
    pub address_length: u8,
    /// deposit amount
    pub deposit_quantity: u64,
}

impl RollupParams {
    pub fn get_deposit_config(&self) -> DepositTxParams {
        DepositTxParams {
            magic_bytes: self.rollup_name.clone().into_bytes().to_vec(),
            address_length: self.address_length,
            deposit_quantity: self.deposit_amount,
        }
    }
}

/// Describes how proofs are generated.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofPublishMode {
    /// Timeout in secs after which a blank proof is generated.
    Timeout(u64),

    /// Expect and wait for non-empty proofs
    Strict,
}

/// Client sync parameters that are used to make the network work but don't
/// strictly have to be pre-agreed.  These have to do with grace periods in
/// message delivery and whatnot.
#[derive(Clone, Debug)]
pub struct SyncParams {
    /// Number of blocks that we follow the L1 from.
    pub l1_follow_distance: u64,

    /// Number of events after which we checkpoint the client
    pub client_checkpoint_interval: u32,

    /// Max number of recent l2 blocks that can be fetched from RPC
    pub l2_blocks_fetch_limit: u64,
}

/// Combined set of parameters across all the consensus logic.
#[derive(Clone, Debug)]
pub struct Params {
    pub rollup: RollupParams,
    pub run: SyncParams,
}

impl Params {
    pub fn rollup(&self) -> &RollupParams {
        &self.rollup
    }

    pub fn run(&self) -> &SyncParams {
        &self.run
    }
}

/// Describes how we determine the list of operators at genesis.
// TODO improve how this looks when serialized
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorConfig {
    /// Use this static list of predetermined operators.
    Static(Vec<OperatorPubkeys>),
}
