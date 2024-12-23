mod manager;
mod pending_tasks;
mod shutdown;

pub use manager::{init_task_manager, TaskError, TaskExecutor, TaskManager};
pub use shutdown::{ShutdownGuard, ShutdownSignal};
