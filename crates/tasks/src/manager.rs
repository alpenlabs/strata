use std::{
    any::Any,
    fmt::{Display, Formatter},
    future::Future,
    panic,
    pin::pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use futures_util::{future::select, FutureExt, TryFutureExt};
use tokio::{runtime::Handle, sync::mpsc};
use tracing::{debug, error, info, warn};

use crate::shutdown::{Shutdown, ShutdownGuard, ShutdownSignal};

/// Error with the name of the task that panicked and an error downcasted to string, if possible.
#[derive(Debug, thiserror::Error)]
pub struct PanickedTaskError {
    task_name: String,
    error: Option<String>,
}

impl Display for PanickedTaskError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let task_name = &self.task_name;
        if let Some(error) = &self.error {
            write!(f, "Critical task `{task_name}` panicked: `{error}`")
        } else {
            write!(f, "Critical task `{task_name}` panicked")
        }
    }
}

impl PanickedTaskError {
    fn new(task_name: &str, error: Box<dyn Any>) -> Self {
        let error = match error.downcast::<String>() {
            Ok(value) => Some(*value),
            Err(error) => match error.downcast::<&str>() {
                Ok(value) => Some(value.to_string()),
                Err(_) => None,
            },
        };

        Self {
            task_name: task_name.to_string(),
            error,
        }
    }
}

/// [`TaskManager`] spawns and tracks long running tasks,
/// watches for task panics and manages graceful shutdown
/// on critical task panics and external signals.
pub struct TaskManager {
    /// Tokio's runtime [`Handle`].
    tokio_handle: Handle,
    /// Channel's sender tasked with sending `panic` signals from tasks.
    panicked_tasks_tx: mpsc::UnboundedSender<PanickedTaskError>,
    /// Channel's receiver tasked with receiving `panic` signals from tasks.
    panicked_tasks_rx: mpsc::UnboundedReceiver<PanickedTaskError>,
    /// Async-capable shutdown signal that can be sent to tasks.
    shutdown_signal: ShutdownSignal,
    /// Pending tasks atomic counter for graceful shutdown.
    pending_tasks_counter: Arc<AtomicUsize>,
}

impl TaskManager {
    pub fn new(tokio_handle: Handle) -> Self {
        let (panicked_tasks_tx, panicked_tasks_rx) = mpsc::unbounded_channel();

        Self {
            tokio_handle,
            panicked_tasks_tx,
            panicked_tasks_rx,
            shutdown_signal: ShutdownSignal::new(),
            pending_tasks_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn executor(&self) -> TaskExecutor {
        TaskExecutor::new(
            self.tokio_handle.clone(),
            self.panicked_tasks_tx.clone(),
            self.shutdown_signal.clone(),
            self.pending_tasks_counter.clone(),
        )
    }

    /// waits until any tasks panic, returns `Err(first_panic_error)`
    /// returns `Ok(())` if shutdown message is received instead
    fn wait_for_task_panic(&mut self, shutdown: Shutdown) -> Result<(), PanickedTaskError> {
        self.tokio_handle.block_on(async {
            tokio::select! {
                msg = self.panicked_tasks_rx.recv() => {
                    match msg {
                        Some(error) => Err(error),
                        None => Ok(())
                    }
                }
                _ = shutdown.wait_for_shutdown() => {
                    Ok(())
                }
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
    fn wait_for_graceful_shutdown(self, timeout: Option<Duration>) -> bool {
        let when = timeout.map(|t| std::time::Instant::now() + t);
        while self.pending_tasks_counter.load(Ordering::Relaxed) > 0 {
            if when
                .map(|when| std::time::Instant::now() > when)
                .unwrap_or(false)
            {
                debug!("graceful shutdown timed out");
                return false;
            }
            std::hint::spin_loop();
        }

        debug!("gracefully shut down");
        true
    }

    /// Add signal listeners and send shutdown
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

    pub fn monitor(mut self, shutdown_timeout: Option<Duration>) -> Result<(), PanickedTaskError> {
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
#[derive(Debug)]
pub struct TaskExecutor {
    /// Handle to the tokio runtime.
    tokio_handle: Handle,
    /// Sender half for sending panic signals from tasks
    panicked_tasks_tx: mpsc::UnboundedSender<PanickedTaskError>,
    /// send shutdown signals to tasks
    shutdown_signal: ShutdownSignal,
    /// number of pending tasks
    pending_tasks_counter: Arc<AtomicUsize>,
}

impl TaskExecutor {
    fn new(
        tokio_handle: Handle,
        panicked_tasks_tx: mpsc::UnboundedSender<PanickedTaskError>,
        shutdown_signal: ShutdownSignal,
        pending_tasks_counter: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            tokio_handle,
            panicked_tasks_tx,
            shutdown_signal,
            pending_tasks_counter,
        }
    }

    /// Spawn task in new thread.
    /// Should check `ShutdownGuard` passed to closure to trigger own shutdown.
    /// Panic will trigger shutdown.
    pub fn spawn_critical<F>(&self, name: &'static str, func: F) -> JoinHandle<()>
    where
        F: FnOnce(ShutdownGuard) + Send + 'static,
    {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let shutdown = ShutdownGuard::new(
            self.shutdown_signal.subscribe(),
            self.pending_tasks_counter.clone(),
        );

        info!(%name, "Starting critical task");
        std::thread::spawn(move || {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| func(shutdown)));

            if let Err(error) = result {
                // TODO: transfer stacktrace?
                let task_error = PanickedTaskError::new(name, error);
                error!(%name, err = %task_error, "critical task failed");
                let _ = panicked_tasks_tx.send(task_error);
            };
        })
    }

    /// Spawn future as task inside tokio runtime.
    /// Panic will trigger shutdown.
    pub fn spawn_critical_async(
        &self,
        name: &'static str,
        fut: impl Future<Output = ()> + Send + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let shutdown = self.shutdown_signal.subscribe();

        // wrap the task in catch unwind
        let task = panic::AssertUnwindSafe(fut)
            .catch_unwind()
            .map_err(move |error| {
                let task_error = PanickedTaskError::new(name, error);
                error!(%name, err = %task_error, "critical task failed");
                let _ = panicked_tasks_tx.send(task_error);
            })
            .map(drop);

        let task = async move {
            // Create an instance of IncCounterOnDrop with the counter to increment
            let task = pin!(task);
            let shutdown = pin!(shutdown.wait_for_shutdown());
            let _ = select(shutdown, task).await;
        };
        info!(%name, "Starting critical async task");
        self.tokio_handle.spawn(task)
    }

    /// Spawn future in tokio runtime.
    /// Should check `ShutdownGuard` passed to closure to trigger own shutdown manually.
    /// Panic will trigger shutdown.
    pub fn spawn_critical_async_with_shutdown<F>(
        &self,
        name: &'static str,
        async_func: impl FnOnce(ShutdownGuard) -> F,
    ) -> tokio::task::JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
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
            .map_err(move |error| {
                let task_error = PanickedTaskError::new(name, error);
                error!(%name, err = %task_error, "critical task failed");
                let _ = panicked_tasks_tx.send(task_error);
            })
            .map(drop);

        self.tokio_handle.spawn(task)
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

        println!("{:#?}", manager.pending_tasks_counter);

        let err = manager
            .monitor(Some(Duration::from_secs(5)))
            .expect_err("should give error");

        panic::set_hook(original_hook);

        assert_eq!(err.task_name, "panictask");
        assert_eq!(err.error, Some("intentional panic".to_string()));
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

        executor.spawn_critical("ok-task", |shutdown| {
            loop {
                if shutdown.should_shutdown() {
                    println!("got shutdown signal");
                    break;
                }

                // doing something useful
                std::thread::sleep(Duration::from_millis(100));
            }
        });

        executor.spawn_critical_async("panictask", async {
            panic!("intentional panic");
        });

        println!("{:#?}", manager.pending_tasks_counter);

        let err = manager
            .monitor(Some(Duration::from_secs(5)))
            .expect_err("should give error");

        panic::set_hook(original_hook);

        assert_eq!(err.task_name, "panictask");
        assert_eq!(err.error, Some("intentional panic".to_string()));
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
                break;
            }

            // doing something useful
            std::thread::sleep(Duration::from_millis(100));
        });

        executor.spawn_critical_async("async-task", async {
            // doing something useful
            std::thread::sleep(Duration::from_millis(100));
        });

        executor.spawn_critical_async_with_shutdown("async-task-2", |shutdown| async move {
            loop {
                if shutdown.should_shutdown() {
                    println!("got shutdown signal");
                    break;
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
                break;
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
                    break;
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
