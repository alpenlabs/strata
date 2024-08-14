//! Defines traits/types related to various book-keeping activities that may be necessary while
//! executing bridge duties. This may hook up with a watchdog or observability service in the
//! future.

pub mod checkpoint;
pub mod errors;
pub mod report_status;
