#![cfg_attr(not(test), warn(unused_crate_dependencies))]
mod config;
pub mod constants;
mod precompiles;
mod utils;

pub use config::{set_evm_handles, ExpressEvmConfig};
