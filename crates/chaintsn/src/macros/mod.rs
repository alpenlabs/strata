#[cfg(feature = "fullstd")]
mod macros_fullstd;

#[cfg(feature = "fullstd")]
pub use macros_fullstd::*;

#[cfg(not(feature = "fullstd"))]
mod macros_nostd;

#[cfg(not(feature = "fullstd"))]
pub use macros_nostd::*;
