//! DB operation executor logic.
//!
//! This manages the indirection to spawn async requests onto a threadpool and execute blocking
//! calls locally.

use std::sync::Arc;

use threadpool::ThreadPool;
use tokio::sync::oneshot;
use tracing::*;

use alpen_express_db::{errors::DbError, DbResult};

/// Shim to opaquely execute the operation without being aware of the underlying impl.
pub struct OpShim<T, R> {
    executor_fn: Arc<dyn Fn(T) -> DbResult<R> + Sync + Send + 'static>,
}

impl<T, R> OpShim<T, R>
where
    T: Sync + Send + 'static,
    R: Sync + Send + 'static,
{
    pub fn wrap<F>(op: F) -> Self
    where
        F: Fn(T) -> DbResult<R> + Sync + Send + 'static,
    {
        Self {
            executor_fn: Arc::new(move |t| op(t)),
        }
    }

    /// Executes the operation on the provided thread pool and returns the result over.
    pub async fn exec_async(&self, pool: &ThreadPool, arg: T) -> DbResult<R> {
        let (resp_tx, resp_rx) = oneshot::channel();

        let exec_fn = self.executor_fn.clone();

        pool.execute(move || {
            let res = exec_fn(arg);
            if resp_tx.send(res).is_err() {
                warn!("failed to send response");
            }
        });

        match resp_rx.await {
            Ok(v) => v,
            Err(e) => Err(DbError::Other(format!("{e}"))),
        }
    }

    /// Executes the operation directly.
    pub fn exec_blocking(&self, arg: T) -> DbResult<R> {
        (self.executor_fn)(arg)
    }
}

macro_rules! inst_ops {
    {
        ($base:ty => $tp:ident, $ctx:ident $(<$($tparam:ident: $tpconstr:tt),+>)?) {
            $($iname:ident => $aname:ident, $bname:ident; $arg:ty => $ret:ty),*
        }
    } => {
        impl $base {
            pub fn new $(<$($tparam: $tpconstr + Sync + Send + 'static),+>)? (pool: ThreadPool, ctx: Arc<$ctx $(<$($tparam),+>)?>) -> Self {
                Self {
                    $tp: pool,
                    $(
                        $iname: {
                            let ctx = ctx.clone();
                            OpShim::wrap(move |arg| {
                                $iname(ctx.as_ref(), arg)
                            })
                        }
                    ),*
                }
            }

            $(
                pub async fn $aname(&self, arg: $arg) -> DbResult<$ret> {
                    self.$iname.exec_async(&self.$tp, arg).await
                }

                pub fn $bname(&self, arg: $arg) -> DbResult<$ret> {
                    self.$iname.exec_blocking(arg)
                }
            ),*
        }
    }
}

pub(crate) use inst_ops;
