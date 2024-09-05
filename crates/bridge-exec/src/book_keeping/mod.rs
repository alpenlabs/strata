//! Defines traits/types related to various book-keeping activities that may be necessary while
//! executing bridge duties. This may hook up with a watchdog or observability service in the
//! future to identify pending deposits, withdrawals, etc.

pub mod errors;
pub mod tracker;
