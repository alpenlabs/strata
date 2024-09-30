mod manager;
mod pending_tasks;
mod shutdown;

pub use manager::{TaskError, TaskExecutor, TaskManager};
pub use shutdown::{ShutdownGuard, ShutdownSignal};
