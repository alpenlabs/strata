import utils

def get_fast_batch_settings() -> utils.RollupParamsSettings:
    v = utils.RollupParamsSettings.new_default()
    v.epoch_slots = 5
    v.genesis_trigger = 5
    return v
