use std::sync::Arc;

use reth::revm::primitives::EVMError;
use reth_chainspec::ChainSpec;
use reth_evm::{env::EvmEnv, ConfigureEvm, ConfigureEvmEnv, Database, NextBlockEnvAttributes};
use reth_evm_ethereum::{EthEvm, EthEvmConfig};
use reth_primitives::{Header, TransactionSigned};
use revm::{inspector_handle_register, EvmBuilder};
use revm_primitives::{Address, CfgEnvWithHandlerCfg, HaltReason, HandlerCfg, TxEnv};
// use strata_reth_evm::set_evm_handles;

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

    pub fn inner(&self) -> &EthEvmConfig {
        &self.inner
    }
}

impl ConfigureEvmEnv for StrataEvmConfig {
    type Header = Header;
    type Transaction = TransactionSigned;
    type Error = std::convert::Infallible;
    type TxEnv = TxEnv;
    type Spec = revm_primitives::SpecId;

    fn tx_env(&self, transaction: &Self::Transaction, signer: Address) -> Self::TxEnv {
        self.inner.tx_env(transaction, signer)
    }

    fn evm_env(&self, header: &Self::Header) -> EvmEnv<Self::Spec> {
        self.inner.evm_env(header)
    }

    fn next_evm_env(
        &self,
        parent: &Self::Header,
        attributes: NextBlockEnvAttributes,
    ) -> Result<EvmEnv<Self::Spec>, Self::Error> {
        self.inner.next_evm_env(parent, attributes)
    }
}

impl ConfigureEvm for StrataEvmConfig {
    type Evm<'a, DB: Database + 'a, I: 'a> = EthEvm<'a, I, DB>;
    type EvmError<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    type HaltReason = HaltReason;

    fn evm_with_env<DB: Database>(
        &self,
        db: DB,
        evm_env: EvmEnv<Self::Spec>,
    ) -> Self::Evm<'_, DB, ()> {
        let cfg_env_with_handler_cfg = CfgEnvWithHandlerCfg {
            cfg_env: evm_env.cfg_env,
            handler_cfg: HandlerCfg::new(evm_env.spec),
        };

        EvmBuilder::default()
            .with_db(db)
            .with_cfg_env_with_handler_cfg(cfg_env_with_handler_cfg)
            .with_block_env(evm_env.block_env)
            // .append_handler_register(set_evm_handles)
            .build()
            .into()
    }

    fn evm_with_env_and_inspector<DB, I>(
        &self,
        db: DB,
        evm_env: EvmEnv<Self::Spec>,
        inspector: I,
    ) -> Self::Evm<'_, DB, I>
    where
        DB: Database,
        I: revm::GetInspector<DB>,
    {
        let cfg_env_with_handler_cfg = CfgEnvWithHandlerCfg {
            cfg_env: evm_env.cfg_env,
            handler_cfg: HandlerCfg::new(evm_env.spec),
        };

        EvmBuilder::default()
            .with_db(db)
            .with_external_context(inspector)
            .with_cfg_env_with_handler_cfg(cfg_env_with_handler_cfg)
            .with_block_env(evm_env.block_env)
            // .append_handler_register(set_evm_handles)
            .append_handler_register(inspector_handle_register)
            .build()
            .into()
    }
}
