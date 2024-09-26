use bitcoin::params::Params;

/// Retrieves the [`Params`] for the Bitcoin network being used.
///
/// This method avoids creating a new struct because [`Params`] is marked as `non_exhaustive`.
///
/// # Note
/// If adjustments to the parameters are required, modify them as shown below:
/// ```
/// let mut btc_params = Params::MAINNET;
/// btc_params.pow_target_spacing = 25 * 30; // Adjusted to 2.5 minutes
/// btc_params
/// ```
pub fn get_btc_params() -> Params {
    Params::MAINNET
}
