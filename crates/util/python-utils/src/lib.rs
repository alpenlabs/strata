use pyo3::prelude::*;

mod constants;
mod drt;
mod error;
mod parse;
mod taproot;

use drt::deposit_request_transaction;
use taproot::{extract_p2tr_pubkey, get_address, get_change_address, musig_aggregate_pks};

/// A Python module implemented in Rust. The name of this function must match
/// the `lib.name` setting in the `Cargo.toml`, else Python will not be able to
/// import the module.
#[pymodule]
fn strata_utils(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(deposit_request_transaction, m)?)?;
    m.add_function(wrap_pyfunction!(get_address, m)?)?;
    m.add_function(wrap_pyfunction!(get_change_address, m)?)?;
    m.add_function(wrap_pyfunction!(musig_aggregate_pks, m)?)?;
    m.add_function(wrap_pyfunction!(extract_p2tr_pubkey, m)?)?;

    Ok(())
}
