use std::marker::PhantomData;

use strata_db::traits::Database;
use strata_status::StatusChannel;
use strata_tasks::TaskManager;

use crate::{
    Client, CsmHandle, SideCar,
    component::ComponentBuilder,
    context::{BuildContext, RunContext},
};

pub struct ClientBuilder<
    LR: ComponentBuilder,
    F: ComponentBuilder,
    C: ComponentBuilder,
    Ch: ComponentBuilder,
> {
    reader: LR,
    fcm: F,
    // rpc: B,
    csm: C,
    chain: Ch,
    sidecars: Vec<Box<dyn SideCar>>,
}

impl Default for ClientBuilder<(), (), (), ()> {
    fn default() -> Self {
        Self {
            reader: (),
            // writer: (),
            fcm: (),
            // rpc: (),
            csm: (),
            chain: (),
            sidecars: Default::default(),
        }
    }
}

impl<LR: ComponentBuilder, F: ComponentBuilder, C: ComponentBuilder, Ch: ComponentBuilder>
    ClientBuilder<LR, F, C, Ch>
{
    pub fn with_reader<R: ComponentBuilder>(self, reader: R) -> ClientBuilder<R, F, C, Ch> {
        ClientBuilder {
            reader,
            fcm: self.fcm,
            csm: self.csm,
            chain: self.chain,
            sidecars: self.sidecars,
        }
    }

    pub fn with_fcm<NewF: ComponentBuilder>(self, fcm: NewF) -> ClientBuilder<LR, NewF, C, Ch> {
        ClientBuilder {
            fcm,
            reader: self.reader,
            csm: self.csm,
            chain: self.chain,
            sidecars: self.sidecars,
        }
    }

    pub fn with_csm<NewC: ComponentBuilder>(self, csm: NewC) -> ClientBuilder<LR, F, NewC, Ch> {
        ClientBuilder {
            csm,
            fcm: self.fcm,
            reader: self.reader,
            chain: self.chain,
            sidecars: self.sidecars,
        }
    }

    pub fn with_chain<NewC: ComponentBuilder>(self, chain: NewC) -> ClientBuilder<LR, F, C, NewC> {
        ClientBuilder {
            chain,
            csm: self.csm,
            fcm: self.fcm,
            reader: self.reader,
            sidecars: self.sidecars,
        }
    }

    pub fn with_sidecar<Sc: SideCar + 'static>(
        mut self,
        sidecar: Sc,
    ) -> ClientBuilder<LR, F, C, Ch> {
        self.sidecars.push(Box::new(sidecar));
        self
    }

    pub fn build_and_validate<
        CH: CsmHandle,
        Cl: Client<LR::Output, F::Output, C::Output, Ch::Output>,
    >(
        &self,
        buildctx: BuildContext,
        task_manager: TaskManager,
        status_channel: StatusChannel,
    ) -> (Cl, RunContext<CH>) {
        let reader = self.reader.build(&buildctx);
        let fcm = self.fcm.build(&buildctx);
        let csm = self.csm.build(&buildctx);
        let chain = self.chain.build(&buildctx);
        // TODO: Sidecars
        // TODO: initialize other things to create runcontext
        let sidecars = Vec::new();
        let client = Cl::from_components(reader, fcm, csm, chain, sidecars);

        // TODO: validate

        let (csm_tx, csm_rx) = mpsc::channel::<CsmMessage>(64);
        let csm_ctl = Arc::new(CsmController::new(database.clone(), pool, csm_tx));

        let runctx = RunContext {
            config: buildctx.config,
            params: buildctx.params,
            db_manager: buildctx.db_manager,
            task_manager,
            status_channel,
            csm_ctl,
        };
        (client, runctx)
    }
}
