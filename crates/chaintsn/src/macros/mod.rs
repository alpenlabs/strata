#[cfg(feature = "std")]
mod macros_std;

#[cfg(feature = "std")]
pub use macros_std::*;

#[cfg(not(feature = "std"))]
mod macros_nostd;

#[cfg(not(feature = "std"))]
pub use macros_nostd::*;
