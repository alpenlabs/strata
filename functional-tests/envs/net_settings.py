from utils import RollupParamsSettings


def get_fast_batch_settings() -> RollupParamsSettings:
    v = RollupParamsSettings.new_default()
    v.epoch_slots = 5
    v.genesis_trigger = 5
    return v
