from utils import RollupParamsSettings


def get_fast_batch_settings() -> RollupParamsSettings:
    v = RollupParamsSettings.new_default().fast_batch()
    return v
