// TODO should be eventually reconciled with AdditionalArgs from strata-reth.
#[derive(Debug, Clone, Default)]
pub struct StrataNodeArgs {
    pub sequencer_http: Option<String>,
}
