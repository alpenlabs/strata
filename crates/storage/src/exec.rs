//! DB operation interface logic, primarily the macro defined to
//!
//! This manages the indirection to spawn async requests onto a threadpool and execute blocking
//! calls locally.

pub use strata_db::{errors::DbError, DbResult};
pub use tracing::*;

/// Handle for receiving a result from a database operation on another thread.
pub type DbRecv<T> = tokio::sync::oneshot::Receiver<DbResult<T>>;

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
                        let resp_rx = self.inner. [<$iname _chan>] (&self.pool, $($aname),*);
                        match resp_rx.await {
                            Ok(v) => v,
                            Err(_e) => Err(DbError::WorkerFailedStrangely),
                        }
                    }

                    pub fn [<$iname _blocking>] (&self, $($aname: $aty),*) -> DbResult<$ret> {
                        self.inner. [<$iname _blocking>] ($($aname),*)
                    }

                    pub fn [<$iname _chan>] (&self, $($aname: $aty),*) -> DbRecv<$ret> {
                        self.inner. [<$iname _chan>] (&self.pool, $($aname),*)
                    }
                )*
            }

            #[async_trait::async_trait]
            trait ShimTrait: Sync + Send + 'static {
                $(
                    fn [<$iname _blocking>] (&self, $($aname: $aty),*) -> DbResult<$ret>;
                    fn [<$iname _chan>] (&self, pool: &threadpool::ThreadPool, $($aname: $aty),*) -> DbRecv<$ret>;
                )*
            }

            pub struct Inner $(<$($tparam: $tpconstr + Sync + Send + 'static),+>)? {
                ctx: Arc<$ctx $(<$($tparam),+>)?>,
            }

            impl $(<$($tparam: $tpconstr + Sync + Send + 'static),+>)? ShimTrait for Inner $(<$($tparam),+>)? {
                $(
                    fn [<$iname _blocking>] (&self, $($aname: $aty),*) -> DbResult<$ret> {
                        $iname(&self.ctx, $($aname),*)
                    }

                    fn [<$iname _chan>] (&self, pool: &threadpool::ThreadPool, $($aname: $aty),*) -> DbRecv<$ret> {
                        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                        let ctx = self.ctx.clone();

                        pool.execute(move || {
                            let res = $iname(&ctx, $($aname),*);
                            if resp_tx.send(res).is_err() {
                                warn!("failed to send response");
                            }
                        });

                        resp_rx
                    }
                )*
            }
        }
    }
}

macro_rules! inst_ops_simple {
    {
        (< $tparam:ident: $tpconstr:tt > => $base:ident) {
            $($iname:ident($($aname:ident: $aty:ty),*) => $ret:ty;)*
        }
    } => {
        pub struct Context<$tparam : $tpconstr> {
            db: Arc<$tparam>,
        }

        impl<$tparam : $tpconstr + Sync + Send + 'static> Context<$tparam> {
            pub fn new(db: Arc<$tparam>) -> Self {
                Self { db }
            }

            pub fn into_ops(self, pool: threadpool::ThreadPool) -> $base {
                $base::new(pool, Arc::new(self))
            }
        }

        inst_ops! {
            ($base, Context<$tparam : $tpconstr>) {
                $($iname ($($aname : $aty ),*) => $ret ;)*
            }
        }

        $(
            inst_ops_ctx_shim!($iname<$tparam: $tpconstr>($($aname: $aty),*) -> $ret);
        )*
    }
}

macro_rules! inst_ops_ctx_shim {
    ($iname:ident<$tparam: ident : $tpconstr:tt>($($aname:ident: $aty:ty),*) -> $ret:ty) => {
        fn $iname < $tparam : $tpconstr > (context: &Context<$tparam>, $($aname : $aty),* ) -> DbResult<$ret> {
            context.db.as_ref(). $iname ( $($aname),* )
        }
    }
}

pub(crate) use inst_ops;
pub(crate) use inst_ops_ctx_shim;
pub(crate) use inst_ops_simple;
