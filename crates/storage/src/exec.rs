//! DB operation executor logic, mainly the macro that generates the IO stuff.
//!
//! This manages the indirection to spawn async requests onto a threadpool and execute blocking
//! calls locally.

use std::sync::Arc;

pub use alpen_express_db::{errors::DbError, DbResult};
pub use tracing::*;

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
    pub async fn exec_async(&self, pool: &threadpool::ThreadPool, arg: T) -> DbResult<R> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();

        let exec_fn = self.executor_fn.clone();

        pool.execute(move || {
            let res = exec_fn(arg);
            if resp_tx.send(res).is_err() {
                tracing::warn!("failed to send response");
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
        ($base:ident, $ctx:ident $(<$($tparam:ident: $tpconstr:tt),+>)?) {
            $($iname:ident($($aname:ident: $aty:ty),*) => $ret:ty;)*
        }
    } => {
        pub struct $base {
            pool: threadpool::ThreadPool,
            inner: Arc<dyn ShimTrait>,
        }

        paste::paste! {
            impl $base {
                pub fn new $(<$($tparam: $tpconstr + Sync + Send + 'static),+>)? (pool: threadpool::ThreadPool, ctx: Arc<$ctx $(<$($tparam),+>)?>) -> Self {
                    Self {
                        pool,
                        inner: Arc::new(Inner { ctx }),
                    }
                }

                $(
                    pub async fn [<$iname _async>] (&self, $($aname: $aty),*) -> DbResult<$ret> {
                        self.inner. [<$iname _async>] (&self.pool, $($aname),*).await
                    }

                    pub fn [<$iname _blocking>] (&self, $($aname: $aty),*) -> DbResult<$ret> {
                        self.inner. [<$iname _blocking>] ($($aname),*)
                    }
                )*
            }

            #[async_trait::async_trait]
            trait ShimTrait {
                $(
                    async fn [<$iname _async>] (&self, pool: &threadpool::ThreadPool, $($aname: $aty),*) -> DbResult<$ret>;
                    fn [<$iname _blocking>] (&self, $($aname: $aty),*) -> DbResult<$ret>;
                )*
            }

            pub struct Inner $(<$($tparam: $tpconstr + Sync + Send + 'static),+>)? {
                ctx: Arc<$ctx $(<$($tparam),+>)?>,
            }

            #[async_trait::async_trait]
            impl $(<$($tparam: $tpconstr + Sync + Send + 'static),+>)? ShimTrait for Inner $(<$($tparam),+>)? {
                $(
                    async fn [<$iname _async>] (&self, pool: &threadpool::ThreadPool, $($aname: $aty),*) -> DbResult<$ret> {
                        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                        let ctx = self.ctx.clone();

                        pool.execute(move || {
                            let res = $iname(&ctx, $($aname),*);
                            if resp_tx.send(res).is_err() {
                                warn!("failed to send response");
                            }
                        });

                        match resp_rx.await {
                            Ok(v) => v,
                            Err(e) => Err(DbError::Other(format!("{e}"))),
                        }
                    }

                    fn [<$iname _blocking>] (&self, $($aname: $aty),*) -> DbResult<$ret> {
                        $iname(&self.ctx, $($aname),*)
                    }
                )*
            }
        }
    }
}

pub(crate) use inst_ops;
