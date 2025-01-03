//! Collection of generic internal data types that are used widely.

// TODO import address types
// TODO import generic account types

pub mod block_credential;
pub mod bridge;
pub mod buf;
pub mod constants;
pub mod errors;
pub mod evm_exec;
pub mod hash;
pub mod l1;
pub mod l2;
#[macro_use]
mod macros;
pub mod keys;
pub mod operator;
pub mod params;
pub mod prelude;
pub mod proof;
pub mod relay;
pub mod sorted_vec;
pub mod utils;
