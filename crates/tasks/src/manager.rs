use std::{
    any::Any,
    fmt::{Display, Formatter},
    future::Future,
    panic::{self, AssertUnwindSafe},
    pin::pin,
    sync::Arc,
    time::Duration,
};

use futures_util::{future::select, FutureExt, TryFutureExt};
use tokio::{runtime::Handle, sync::mpsc};
use tracing::{debug, error, info, warn};

use crate::{
    pending_tasks::PendingTasks,
    shutdown::{Shutdown, ShutdownGuard, ShutdownSignal},
};

#[derive(Debug, thiserror::Error)]
enum FailureReason {
    #[error("panic: {0}")]
    Panic(String),
    #[error("error: {0}")]
    Err(#[source] anyhow::Error),
}

/// Error with the name of the task that panicked and an error downcasted to string, if possible.
#[derive(Debug)]
pub struct TaskError {
    task_name: String,
    reason: FailureReason,
}

impl Display for TaskError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let task_name = &self.task_name;
        match &self.reason {
            FailureReason::Err(error) => {
                write!(f, "Critical task `{task_name}` ended with err: `{error}`")
            }
            FailureReason::Panic(error) => {
                write!(f, "Critical task `{task_name}` panicked: `{error}`")
            }
        }
    }
}

impl TaskError {
    fn from_panic(task_name: &str, error: Box<dyn Any>) -> Self {
        let error_message = match error.downcast::<String>() {
            Ok(value) => Some(*value),
            Err(error) => match error.downcast::<&str>() {
                Ok(value) => Some(value.to_string()),
                Err(_) => None,
            },
        };

        Self {
            task_name: task_name.to_string(),
            reason: FailureReason::Panic(error_message.unwrap_or_default()),
        }
    }

    fn from_err(task_name: &str, err: anyhow::Error) -> Self {
        Self {
            task_name: task_name.to_string(),
            reason: FailureReason::Err(err),
        }
    }
}

impl From<TaskError> for anyhow::Error {
    fn from(value: TaskError) -> Self {
        match value.reason {
            FailureReason::Err(error) => error,
            FailureReason::Panic(panic_message) => anyhow::Error::msg(panic_message),
        }
        .context(value.task_name)
    }
}

/// [`TaskManager`] spawns and tracks long running tasks,
/// watches for task panics and manages graceful shutdown
/// on critical task panics and external signals.
pub struct TaskManager {
    /// Tokio's runtime [`Handle`].
    tokio_handle: Handle,
    /// Channel's sender tasked with sending `panic` signals from tasks.
    critical_task_end_tx: mpsc::UnboundedSender<TaskError>,
    /// Channel's receiver tasked with receiving `panic` signals from tasks.
    critical_task_end_rx: mpsc::UnboundedReceiver<TaskError>,
    /// Async-capable shutdown signal that can be sent to tasks.
    shutdown_signal: ShutdownSignal,
    /// Pending tasks atomic counter for graceful shutdown.
    pending_tasks_counter: Arc<PendingTasks>,
}

impl TaskManager {
    pub fn new(tokio_handle: Handle) -> Self {
        let (panicked_tasks_tx, panicked_tasks_rx) = mpsc::unbounded_channel();

        Self {
            tokio_handle,
            critical_task_end_tx: panicked_tasks_tx,
            critical_task_end_rx: panicked_tasks_rx,
            shutdown_signal: ShutdownSignal::new(),
            pending_tasks_counter: Arc::new(PendingTasks::new(0)),
        }
    }

    pub fn executor(&self) -> TaskExecutor {
        TaskExecutor::new(
            self.tokio_handle.clone(),
            self.critical_task_end_tx.clone(),
            self.shutdown_signal.clone(),
            self.pending_tasks_counter.clone(),
        )
    }

    pub fn handle(&self) -> &Handle {
        &self.tokio_handle
    }

    /// waits until any tasks panic, returns `Err(first_panic_error)`
    /// returns `Ok(())` if shutdown message is received instead
    fn wait_for_task_panic(&mut self, shutdown: Shutdown) -> Result<(), TaskError> {
        self.tokio_handle.block_on(async {
            tokio::select! {
                msg = self.critical_task_end_rx.recv() => {
                    // critical task errored
                    match msg {
                        Some(error) => Err(error),
                        None => Ok(())
                    }
                }
                _ = shutdown.wait_for_shutdown() => {
                    // got shutdown signal
                    Ok(())
                },
            }
        })
    }

    /// Get shutdown signal trigger
    pub fn shutdown_signal(&self) -> ShutdownSignal {
        self.shutdown_signal.clone()
    }

    /// Wait for all tasks to complete, returning true.
    /// If timeout is provided, wait until timeout;
    /// return false if tasks have not completed by this time.
    fn wait_for_graceful_shutdown(&self, timeout: Option<Duration>) -> bool {
        self.tokio_handle
            .block_on(self.wait_for_graceful_shutdown_async(timeout))
    }

    /// Wait for all tasks to complete, returning true.
    /// If timeout is provided, wait until timeout;
    /// return false if tasks have not completed by this time.
    async fn wait_for_graceful_shutdown_async(&self, timeout: Option<Duration>) -> bool {
        let no_pending_tasks_future = self.pending_tasks_counter.clone().wait_for_zero();

        if let Some(duration) = timeout {
            match tokio::time::timeout(duration, no_pending_tasks_future).await {
                Ok(()) => {
                    debug!("gracefully shut down");
                    true
                }
                Err(_) => {
                    debug!("graceful shutdown timed out");
                    false
                }
            }
        } else {
            no_pending_tasks_future.await;
            debug!("gracefully shut down");
            true
        }
    }

    /// Start background handler to send the shutdown signal when a ctrl-c (SIGINT) is received
    pub fn start_signal_listeners(&self) {
        let shutdown_signal = self.shutdown_signal();

        self.tokio_handle.spawn(async move {
            // TODO: add more relevant signals
            // TODO: double ctrl+c for force quit
            let _ = tokio::signal::ctrl_c().into_future().await;

            // got a ctrl+c. send a shutdown
            warn!("Got INT. Initiating shutdown");
            shutdown_signal.send()
        });
    }

    pub fn monitor(mut self, shutdown_timeout: Option<Duration>) -> Result<(), TaskError> {
        // TODO: shut down if all pending tasks exit without errors
        let res = self.wait_for_task_panic(self.shutdown_signal.subscribe());

        self.shutdown_signal.send();
        let shutdown_in_time = self.wait_for_graceful_shutdown(shutdown_timeout);

        if !shutdown_in_time {
            info!("Shutdown timeout expired; Forced shutdown");
        }

        // join all pending threads ?

        res
    }
}

/// A type that can spawn new tasks
#[derive(Debug, Clone)]
pub struct TaskExecutor {
    /// Handle to the tokio runtime.
    tokio_handle: Handle,
    /// Sender half for sending panic signals from tasks
    panicked_tasks_tx: mpsc::UnboundedSender<TaskError>,
    /// send shutdown signals to tasks
    shutdown_signal: ShutdownSignal,
    /// number of pending tasks
    pending_tasks_counter: Arc<PendingTasks>,
}

impl TaskExecutor {
    fn new(
        tokio_handle: Handle,
        panicked_tasks_tx: mpsc::UnboundedSender<TaskError>,
        shutdown_signal: ShutdownSignal,
        pending_tasks_counter: Arc<PendingTasks>,
    ) -> Self {
        Self {
            tokio_handle,
            panicked_tasks_tx,
            shutdown_signal,
            pending_tasks_counter,
        }
    }

    pub fn handle(&self) -> &Handle {
        &self.tokio_handle
    }

    /// Spawn task in new thread.
    /// Should check `ShutdownGuard` passed to closure to trigger own shutdown.
    /// Panic will trigger shutdown.
    pub fn spawn_critical<F>(&self, name: &'static str, func: F)
    where
        F: FnOnce(ShutdownGuard) -> anyhow::Result<()> + Send + 'static,
    {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let shutdown = ShutdownGuard::new(
            self.shutdown_signal.subscribe(),
            self.pending_tasks_counter.clone(),
        );

        info!(%name, "Starting critical task");
        std::thread::spawn(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(|| func(shutdown)));

            match result {
                Ok(task_result) => {
                    if let Err(e) = task_result {
                        // Log the error with backtrace if available
                        error!(%name, error = %e, "Critical task returned an error");
                        let _ = panicked_tasks_tx.send(TaskError::from_err(name, e));
                    } else {
                        // ended successfully
                        info!(%name, "Critical task ended");
                    }
                }
                Err(panic_err) => {
                    // Task panicked
                    let task_error = TaskError::from_panic(name, panic_err);
                    error!(%name, err = %task_error, "Critical task panicked");
                    let _ = panicked_tasks_tx.send(task_error);
                }
            };
        });
    }

    /// Spawn future as task inside tokio runtime.
    /// Panic will trigger shutdown.
    pub fn spawn_critical_async(
        &self,
        name: &'static str,
        fut: impl Future<Output = anyhow::Result<()>> + Send + 'static,
    ) {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let shutdown = self.shutdown_signal.subscribe();

        // wrap the task in catch unwind
        let task = panic::AssertUnwindSafe(fut)
            .catch_unwind()
            .then(move |result| {
                async move {
                    match result {
                        Ok(task_result) => {
                            if let Err(e) = task_result {
                                // Log the error with backtrace if available
                                error!(%name, error = %e, "Critical async task returned an error");
                                let _ = panicked_tasks_tx.send(TaskError::from_err(name, e));
                            } else {
                                // ended successfully
                                info!(%name, "Critical task ended");
                            }
                        }
                        Err(panic_err) => {
                            // Task panicked
                            let task_error = TaskError::from_panic(name, panic_err);
                            error!(%name, err = %task_error, "Critical async task panicked");
                            let _ = panicked_tasks_tx.send(task_error);
                        }
                    }
                }
            });

        let task = async move {
            // Create an instance of IncCounterOnDrop with the counter to increment
            let task = pin!(task);
            let shutdown_fut = pin!(shutdown.wait_for_shutdown());
            let _ = select(shutdown_fut, task).await;
        };

        info!(%name, "Starting critical async task");
        self.tokio_handle.spawn(task);
    }

    /// Spawn future in tokio runtime.
    /// Should check `ShutdownGuard` passed to closure to trigger own shutdown manually.
    /// Panic will trigger shutdown.
    pub fn spawn_critical_async_with_shutdown<F>(
        &self,
        name: &'static str,
        async_func: impl FnOnce(ShutdownGuard) -> F,
    ) where
        F: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let shutdown = ShutdownGuard::new(
            self.shutdown_signal.subscribe(),
            self.pending_tasks_counter.clone(),
        );
        let fut = async_func(shutdown);

        // wrap the task in catch unwind
        let task = panic::AssertUnwindSafe(fut)
            .catch_unwind()
            .then(move |result| {
                async move {
                    match result {
                        Ok(task_result) => {
                            if let Err(e) = task_result {
                                // Log the error with backtrace if available
                                error!(%name, error = %e, "Critical async task returned an error");
                                let _ = panicked_tasks_tx.send(TaskError::from_err(name, e));
                            } else {
                                // ended successfully
                                info!(%name, "Critical task ended");
                            }
                        }
                        Err(panic_err) => {
                            // Task panicked
                            let task_error = TaskError::from_panic(name, panic_err);
                            error!(%name, err = %task_error, "Critical async task panicked");
                            let _ = panicked_tasks_tx.send(task_error);
                        }
                    }
                }
            })
            .map(drop);

        self.tokio_handle.spawn(task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();
        let manager = TaskManager::new(handle);
        let executor = manager.executor();

        // dont want to print stack trace for expected error while running test
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));

        executor.spawn_critical("panictask", |_| {
            panic!("intentional panic");
        });

        println!("{:#?}", manager.pending_tasks_counter.current());

        let err = manager
            .monitor(Some(Duration::from_secs(5)))
            .expect_err("should give error");

        panic::set_hook(original_hook);

        assert_eq!(err.task_name, "panictask");
        assert!(matches!(
            err.reason,
            FailureReason::Panic(error) if error == *"intentional panic",
        ));
    }

    #[test]
    fn test_critical_async() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();
        let manager = TaskManager::new(handle);
        let executor = manager.executor();

        // dont want to print stack trace for expected error while running test
        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));

        executor.spawn_critical("ok-task", |shutdown| loop {
            if shutdown.should_shutdown() {
                println!("got shutdown signal");
                break Ok(());
            }

            // doing something useful
            std::thread::sleep(Duration::from_millis(100));
        });

        executor.spawn_critical_async("panictask", async {
            panic!("intentional panic");
        });

        eprintln!("{:#?}", manager.pending_tasks_counter);

        let err = manager
            .monitor(Some(Duration::from_secs(5)))
            .expect_err("should give error");

        panic::set_hook(original_hook);

        assert_eq!(err.task_name, "panictask");
        assert!(matches!(
            err.reason,
            FailureReason::Panic(error) if error == "intentional panic",
        ));
    }

    #[test]
    fn test_shutdown() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();
        let manager = TaskManager::new(handle);
        let executor = manager.executor();

        executor.spawn_critical("task", |shutdown| loop {
            if shutdown.should_shutdown() {
                println!("got shutdown signal");
                break Ok(());
            }

            // doing something useful
            std::thread::sleep(Duration::from_millis(100));
        });

        executor.spawn_critical_async("async-task", async {
            // doing something useful
            std::thread::sleep(Duration::from_millis(100));
            Ok(())
        });

        executor.spawn_critical_async_with_shutdown("async-task-2", |shutdown| async move {
            loop {
                if shutdown.should_shutdown() {
                    println!("got shutdown signal");
                    break Ok(());
                }

                // doing something useful
                std::thread::sleep(Duration::from_millis(100));
            }
        });

        let shutdown_sig = manager.shutdown_signal.clone();

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            shutdown_sig.send();
        });

        let res = manager.monitor(Some(Duration::from_secs(5)));

        assert!(matches!(res, Ok(())), "should exit successfully");
    }

    #[test]
    fn test_shutdown_critical() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();
        let mut manager = TaskManager::new(handle);
        let executor = manager.executor();

        executor.spawn_critical("task", |shutdown| loop {
            if shutdown.should_shutdown() {
                println!("got shutdown signal");
                break Ok(());
            }

            // doing something useful
            std::thread::sleep(Duration::from_millis(100));
        });

        let shutdown_sig = manager.shutdown_signal.clone();

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            shutdown_sig.send();
        });

        let _ = manager.wait_for_task_panic(manager.shutdown_signal().subscribe());
        let shutdown_in_time = manager.wait_for_graceful_shutdown(Some(Duration::from_secs(5)));

        assert!(shutdown_in_time, "should exit successfully in time");
    }

    #[test]
    fn test_shutdown_critical_async() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();
        let mut manager = TaskManager::new(handle);
        let executor = manager.executor();

        executor.spawn_critical_async("async-task", async {
            // doing something useful
            std::thread::sleep(Duration::from_millis(100));
            Ok(())
        });

        let shutdown_sig = manager.shutdown_signal.clone();

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            shutdown_sig.send();
        });

        let _ = manager.wait_for_task_panic(manager.shutdown_signal().subscribe());
        let shutdown_in_time = manager.wait_for_graceful_shutdown(Some(Duration::from_secs(5)));

        assert!(shutdown_in_time, "should exit successfully in time");
    }

    #[test]
    fn test_shutdown_critical_async_with_shutdown() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();
        let mut manager = TaskManager::new(handle);
        let executor = manager.executor();

        executor.spawn_critical_async_with_shutdown("async-task-2", |shutdown| async move {
            loop {
                if shutdown.should_shutdown() {
                    println!("got shutdown signal");
                    break Ok(());
                }

                // doing something useful
                std::thread::sleep(Duration::from_millis(100));
            }
        });

        let shutdown_sig = manager.shutdown_signal.clone();

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            shutdown_sig.send();
        });

        let _ = manager.wait_for_task_panic(manager.shutdown_signal().subscribe());
        let shutdown_in_time = manager.wait_for_graceful_shutdown(Some(Duration::from_secs(5)));

        assert!(shutdown_in_time, "should exit successfully in time");
    }
}
