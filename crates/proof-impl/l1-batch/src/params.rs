use bitcoin::params::Params;

/// Retrieves the [`Params`] for the Bitcoin network being used.
///
/// This method avoids creating a new struct because [`Params`] is marked as `non_exhaustive`.
///
/// # Note
/// If adjustments to the parameters are required, modify them as shown below:
/// ```
/// use bitcoin::params::Params;
/// fn get_btc_params() -> Params {
///     let mut btc_params: Params = Params::MAINNET;
///     btc_params.pow_target_spacing = 25 * 30; // Adjusted to 2.5 minutes
///     btc_params // Return the modified `btc_params`
/// }
/// ```
///
/// # Returns
/// Returns the default Bitcoin Parameters used in our rollup.
pub fn get_btc_params() -> Params {
    Params::MAINNET
}
