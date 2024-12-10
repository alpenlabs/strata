//! Utils for reasoning about chain slots.

use strata_primitives::params::RollupParams;

pub fn get_slot_expected_epoch(slot: u64, params: &RollupParams) -> u64 {
    slot / params.target_l2_batch_size as u64
}

pub fn get_epoch_initial_slot(epoch: u64, params: &RollupParams) -> u64 {
    epoch * params.target_l2_batch_size
}

pub fn get_epoch_final_slot(epoch: u64, params: &RollupParams) -> u64 {
    let epoch_init_slot = get_epoch_initial_slot(epoch, params);
    epoch_init_slot + (params.target_l2_batch_size - 1)
}

pub fn is_epoch_init_slot(slot: u64, params: &RollupParams) -> bool {
    let epoch = get_slot_expected_epoch(slot, params);
    slot == get_epoch_initial_slot(epoch, params)
}

pub fn is_epoch_final_slot(slot: u64, params: &RollupParams) -> bool {
    let epoch = get_slot_expected_epoch(slot, params);
    slot == get_epoch_final_slot(epoch, params)
}
