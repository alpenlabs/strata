//! Defines traits related to reporting the status for bridge duty executions and
//! some common implementers. These may tie up to a watchdog or a observability service in the
//! future.

use alpen_express_state::bridge_duties::Duty;
use async_trait::async_trait;

use crate::operator::errors::ExecError;

/// Defines functionalities to report the status of an execution. This could be implemented by types
/// like `ConsoleReporter` or `LogfileReporter`, etc.
#[async_trait]
pub trait ReportStatus: Clone + Send + Sync + Sized {
    /// Report the status of an operation once it is complete.
    async fn report_status(&self, duty: &Duty, status: &str);

    /// Report any errors that may occur during the execution of an operation.
    async fn report_error(&self, duty: &Duty, error: ExecError);
}
