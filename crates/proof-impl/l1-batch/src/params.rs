/// Retrieves the [`bitcoin::params::Params`] for the Bitcoin network being used.
///
/// This method avoids creating a new struct because [`bitcoin::params::Params`] is marked as
/// `non_exhaustive`.
///
/// # Note
/// If adjustments to the parameters are required, modify them as shown below:
/// ```
/// fn get_btc_params() -> bitcoin::params::Params {
///     let mut btc_params = bitcoin::params::MAINNET.clone();
///     btc_params.pow_target_spacing = 25 * 30; // Adjusted to 2.5 minutes
///     btc_params // Return the modified `btc_params`
/// }
/// ```
///
/// # Returns
/// Returns the default Bitcoin Parameters used in our rollup.
pub fn get_btc_params() -> bitcoin::params::Params {
    bitcoin::params::Params::MAINNET
}
