from utils import RollupParamsSettings


def get_fast_batch_settings() -> RollupParamsSettings:
    v = RollupParamsSettings.new_default()
    v.proof_timeout = 1
    return v
