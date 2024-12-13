use std::sync::Arc;

use reth_chainspec::ChainSpec;
use reth_evm::{ConfigureEvm, ConfigureEvmEnv, NextBlockEnvAttributes};
use reth_node_ethereum::EthEvmConfig;
use reth_primitives::{Header, TransactionSigned};
use revm_primitives::{
    Address, AnalysisKind, BlockEnv, Bytes, CfgEnvWithHandlerCfg, Env, TxEnv, U256,
};

/// Custom EVM configuration
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StrataEvmConfig {
    inner: EthEvmConfig,
}

impl StrataEvmConfig {
    pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            inner: EthEvmConfig::new(chain_spec),
        }
    }
}

impl ConfigureEvmEnv for StrataEvmConfig {
    type Header = Header;
    type Transaction = TransactionSigned;
    type Error = core::convert::Infallible;

    fn fill_cfg_env(
        &self,
        cfg_env: &mut CfgEnvWithHandlerCfg,
        header: &Self::Header,
        total_difficulty: U256,
    ) {
        self.inner.fill_cfg_env(cfg_env, header, total_difficulty);
        // TODO: check if it's still needed.
        cfg_env.perf_analyse_created_bytecodes = AnalysisKind::Analyse;
    }

    fn fill_tx_env(&self, tx_env: &mut TxEnv, transaction: &TransactionSigned, sender: Address) {
        self.inner.fill_tx_env(tx_env, transaction, sender);
    }

    fn fill_tx_env_system_contract_call(
        &self,
        env: &mut Env,
        caller: Address,
        contract: Address,
        data: Bytes,
    ) {
        self.inner
            .fill_tx_env_system_contract_call(env, caller, contract, data);
    }

    fn next_cfg_and_block_env(
        &self,
        parent: &Self::Header,
        attributes: NextBlockEnvAttributes,
    ) -> Result<(CfgEnvWithHandlerCfg, BlockEnv), Self::Error> {
        self.inner.next_cfg_and_block_env(parent, attributes)
    }
}

impl ConfigureEvm for StrataEvmConfig {
    type DefaultExternalContext<'a> = ();

    #[doc = " Provides the default external context."]
    fn default_external_context<'a>(&self) -> Self::DefaultExternalContext<'a> {
        self.inner.default_external_context()
    }
}
