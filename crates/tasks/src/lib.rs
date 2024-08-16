use std::any::Any;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;
use std::thread::JoinHandle;
use std::time::Duration;

use futures_util::future::poll_fn;
use futures_util::{FutureExt, TryFutureExt};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tracing::{debug, error};

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
        let error = match error.downcast::<PanickedTaskError>() {
            Ok(value) => return *value,
            Err(error) => match error.downcast::<String>() {
                Ok(value) => Some(*value),
                Err(error) => match error.downcast::<&str>() {
                    Ok(value) => Some(value.to_string()),
                    Err(_) => None,
                },
            },
        };

        Self {
            task_name: task_name.to_string(),
            error,
        }
    }
}

#[derive(Debug, Clone)]
struct ShutdownSignal(Arc<AtomicBool>);

impl ShutdownSignal {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    fn send(&self) {
        self.0.fetch_or(true, Ordering::Relaxed);
    }

    fn subscribe(&self) -> Shutdown {
        Shutdown(self.0.clone())
    }
}

struct Shutdown(Arc<AtomicBool>);

impl Shutdown {
    fn should_shutdown(&mut self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    async fn into_future(self) {
        poll_fn(|_| {
            if self.0.load(Ordering::Relaxed) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await
    }
}

pub struct ShutdownGuard(Shutdown, Arc<AtomicUsize>);

impl ShutdownGuard {
    fn new(shutdown: Shutdown, counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(shutdown, counter)
    }
    pub fn should_shutdown(&mut self) -> bool {
        self.0.should_shutdown()
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        self.1.fetch_sub(1, Ordering::SeqCst);
    }
}

// #[derive(Debug)]
// struct NamedJoinHandle<T> {
//     name: &'static str,
//     inner: JoinHandle<T>,
// }

pub struct TaskManager {
    /// Handle to the tokio runtime.
    tokio_handle: Handle,
    /// Sender half for sending panic signals from tasks
    panicked_tasks_tx: mpsc::UnboundedSender<PanickedTaskError>,
    /// Receiver half for sending panic signals to tasks
    panicked_tasks_rx: mpsc::UnboundedReceiver<PanickedTaskError>,
    /// send shutdown signals to tasks
    shutdown_signal: ShutdownSignal,
    /// join handles to active threads
    // join_handles: Arc<Mutex<VecDeque<NamedJoinHandle<()>>>>,
    /// pending tasks count
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
            // join_handles: Arc::new(Mutex::new(VecDeque::new())),
            pending_tasks_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn executor(&self) -> TaskExecutor {
        TaskExecutor::new(
            self.tokio_handle.clone(),
            self.panicked_tasks_tx.clone(),
            self.shutdown_signal.clone(),
            // self.join_handles.clone(),
            self.pending_tasks_counter.clone(),
        )
    }

    fn wait_for_task_panic(&mut self, shutdown: Shutdown) -> Result<(), PanickedTaskError> {
        self.tokio_handle.block_on(async {
            tokio::select! {
                msg = self.panicked_tasks_rx.recv() => {
                    match msg {
                        Some(error) => Err(error),
                        None => Ok(())
                    }
                }
                _ = shutdown.into_future() => {
                    Ok(())
                }
            }
        })
    }

    pub fn graceful_shutdown(self, timeout: Option<Duration>) -> bool {
        self.shutdown_signal.send();
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

    pub fn monitor(mut self) -> Result<(), PanickedTaskError> {
        println!("monitor");
        let res = self.wait_for_task_panic(self.shutdown_signal.subscribe());

        println!("start shutdown");

        self.graceful_shutdown(None);

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
    /// join handles to active threads
    // join_handles: Arc<Mutex<VecDeque<NamedJoinHandle<()>>>>,
    /// number of pending tasks
    pending_tasks_counter: Arc<AtomicUsize>,
}

impl TaskExecutor {
    fn new(
        tokio_handle: Handle,
        panicked_tasks_tx: mpsc::UnboundedSender<PanickedTaskError>,
        shutdown_signal: ShutdownSignal,
        // join_handles: Arc<Mutex<VecDeque<NamedJoinHandle<()>>>>,
        pending_tasks_counter: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            tokio_handle,
            panicked_tasks_tx,
            shutdown_signal,
            // join_handles,
            pending_tasks_counter,
        }
    }

    pub fn spawn_critical<F>(&self, name: &'static str, func: F) -> JoinHandle<()>
    where
        F: FnOnce(ShutdownGuard) + Send + Sync + 'static,
    {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let shutdown = ShutdownGuard::new(
            self.shutdown_signal.subscribe(),
            self.pending_tasks_counter.clone(),
        );
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| func(shutdown)));

            if let Err(error) = result {
                let task_error = PanickedTaskError::new(name, error);
                error!("{task_error}");
                let _ = panicked_tasks_tx.send(task_error);
            };
        })

        // self.join_handles
        //     .lock()
        //     .unwrap()
        //     .push_back(NamedJoinHandle {
        //         name,
        //         inner: handle,
        //     });
    }

    pub fn spawn_critical_async<F>(
        &self,
        name: &'static str,
        async_func: impl FnOnce(ShutdownGuard) -> F,
    ) -> tokio::task::JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let panicked_tasks_tx = self.panicked_tasks_tx.clone();
        let shutdown = ShutdownGuard(
            self.shutdown_signal.subscribe(),
            self.pending_tasks_counter.clone(),
        );
        let fut = async_func(shutdown);

        // wrap the task in catch unwind
        let task = std::panic::AssertUnwindSafe(fut)
            .catch_unwind()
            .map_err(move |error| {
                let task_error = PanickedTaskError::new(name, error);
                error!("{task_error}");
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
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        executor.spawn_critical("panictask", |_| {
            panic!("intentional panic");
        });

        println!("{:#?}", manager.pending_tasks_counter);

        let err = manager.monitor().expect_err("should give error");

        std::panic::set_hook(original_hook);

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
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        executor.spawn_critical("ok-task", |mut shutdown| {
            loop {
                if shutdown.should_shutdown() {
                    println!("got shutdown signal");
                    break;
                }

                // doing something useful
                std::thread::sleep(Duration::from_millis(100));
            }
        });

        executor.spawn_critical_async("panictask", |_| async {
            panic!("intentional panic");
        });

        println!("{:#?}", manager.pending_tasks_counter);

        let err = manager.monitor().expect_err("should give error");

        std::panic::set_hook(original_hook);

        assert_eq!(err.task_name, "panictask");
        assert_eq!(err.error, Some("intentional panic".to_string()));
    }

    #[test]
    fn test_shutdown() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();
        let manager = TaskManager::new(handle);
        let executor = manager.executor();

        executor.spawn_critical("task", |mut shutdown| loop {
            if shutdown.should_shutdown() {
                println!("got shutdown signal");
                break;
            }

            // doing something useful
            std::thread::sleep(Duration::from_millis(100));
        });

        executor.spawn_critical_async("async-task", |mut shutdown| async move {
            loop {
                if shutdown.should_shutdown() {
                    println!("got shutdown signal");
                    break;
                }

                // doing something useful
                std::thread::sleep(Duration::from_millis(100));
            }
        });

        manager.shutdown_signal.send();

        let res = manager.monitor();

        assert!(matches!(res, Ok(())), "should exit successfully");
    }
}
