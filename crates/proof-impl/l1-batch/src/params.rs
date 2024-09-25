use bitcoin::params::Params;

pub fn get_btc_params() -> Params {
    Params::MAINNET

    // Note: Update params if necessary in the following way
    // let mut btc_params = Params::MAINNET;
    // btc_params.pow_target_spacing = 25 * 30; // 2.5 minutes
    // btc_params
}
