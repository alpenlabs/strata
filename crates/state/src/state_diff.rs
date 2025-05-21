use strata_da_lib::diff::*;
use strata_primitives::{
    buf::Buf32,
    epoch::EpochCommitment,
    l1::{payload::BlobSpec, HeaderVerificationState},
    l2::L2BlockCommitment,
};

use crate::{
    bridge_ops::DepositIntent,
    bridge_state::{DepositEntry, OperatorEntry},
    exec_update::{Op, UpdateInputDiff},
    forced_inclusion::ForcedInclusion,
};

pub struct ChainStateDiff {
    curr_slot_diff: Vec<NumDiff<u32, u8>>,
    prev_block_diff: Vec<RegisterDiff<L2BlockCommitment>>,
    cur_epoch_diff: Vec<NumDiff<u64, u8>>,
    prev_epoch_diff: Vec<RegisterDiff<EpochCommitment>>,
    is_epoch_finishing_diff: bool,
    finalized_epoch_diff: Vec<RegisterDiff<EpochCommitment>>,
    l1_state_diff: L1ViewStateDiff,
    pending_withdraws_diff: Vec<RegisterDiff<Buf32>>,
    exec_env_state_diff: ExecEnvStateDiff,
    operator_table_diff: TableDiff<OperatorEntry>,
    deposits_table_diff: TableDiff<DepositEntry>,
}

// TODO: maybe need to add macro to generate *Diff struct for any struct.

pub struct L1ViewStateDiff {
    horizon_height_diff: Vec<RegisterDiff<u64>>,
    genesis_height_diff: Vec<RegisterDiff<u64>>,
    safe_block_height_diff: Vec<RegisterDiff<u64>>,
    heaer_vs_diff: Vec<RegisterDiff<HeaderVerificationState>>,
}

pub struct ExecEnvStateDiff {
    last_update_input_diff: UpdateInputDiff,
    curr_state_diff: Vec<RegisterDiff<Buf32>>,
    waiting_da_blobs_diff: Vec<ListDiff<BlobSpec>>,
    pending_deposits_diff: StateQueueDiff<DepositIntent>,
    pending_forced_incls_diff: StateQueueDiff<ForcedInclusion>,
}

// Maybe put in another crate(da-lib) since this seems to be generic?
pub struct StateQueueDiff<T> {
    base_idx_diff: Vec<RegisterDiff<u64>>,
    entries_diff: Vec<ListDiff<T>>,
}

// Maybe put in another crate(da-lib) since this seems to be generic? Also, looks very similar to
// `StateQueueDiff.
pub struct TableDiff<T> {
    next_idx_diff: Vec<RegisterDiff<u64>>,
    entries_diff: Vec<ListDiff<T>>,
}
