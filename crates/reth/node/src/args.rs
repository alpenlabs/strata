use revm_primitives::Address;

#[derive(Debug, Clone, Default)]
pub struct StrataNodeArgs {
    pub sequencer_http: Option<String>,
    pub enable_eoa: bool,
    pub allowed_eoa_addrs: Vec<Address>,
}
