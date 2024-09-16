use std::sync::Arc;

use reth_chainspec::{ChainSpec, Head};
use reth_evm::{ConfigureEvm, ConfigureEvmEnv};
use reth_node_ethereum::EthEvmConfig;
use reth_primitives::{Header, TransactionSigned};
use revm::{
    handler::register::EvmHandler, inspector_handle_register, precompile::PrecompileSpecId,
    Context, ContextPrecompile, ContextPrecompiles, Database, Evm, EvmBuilder, GetInspector,
};
use revm_primitives::{
    Address, AnalysisKind, Bytes, CfgEnvWithHandlerCfg, EVMError, Env, SpecId, TxEnv, U256,
};

use crate::{
    constants::{BASEFEE_ADDRESS, FIXED_WITHDRAWAL_WEI},
    precompiles,
};

/// Custom EVM configuration
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct ExpressEvmConfig;

impl ExpressEvmConfig {
    /// Sets the precompiles to the EVM handler
    ///
    /// This will be invoked when the EVM is created via [ConfigureEvm::evm] or
    /// [ConfigureEvm::evm_with_inspector]
    ///
    /// This will use the default mainnet precompiles and add additional precompiles.
    pub fn set_precompiles<EXT, DB>(handler: &mut EvmHandler<EXT, DB>)
    where
        DB: Database,
    {
        // first we need the evm spec id, which determines the precompiles
        let spec_id = handler.cfg.spec_id;

        // install the precompiles
        handler.pre_execution.load_precompiles = Arc::new(move || {
            let mut precompiles = ContextPrecompiles::new(PrecompileSpecId::from_spec_id(spec_id));
            precompiles.extend([(
                precompiles::bridge::BRIDGEOUT_ADDRESS,
                ContextPrecompile::ContextStateful(Arc::new(
                    precompiles::bridge::BridgeoutPrecompile::new(FIXED_WITHDRAWAL_WEI),
                )),
            )]);
            precompiles
        });
    }
}

impl ConfigureEvmEnv for ExpressEvmConfig {
    fn fill_cfg_env(
        &self,
        cfg_env: &mut CfgEnvWithHandlerCfg,
        chain_spec: &ChainSpec,
        header: &Header,
        total_difficulty: U256,
    ) {
        let spec_id = reth_evm_ethereum::revm_spec(
            chain_spec,
            &Head {
                number: header.number,
                timestamp: header.timestamp,
                difficulty: header.difficulty,
                total_difficulty,
                hash: Default::default(),
            },
        );

        cfg_env.chain_id = chain_spec.chain().id();
        cfg_env.perf_analyse_created_bytecodes = AnalysisKind::Analyse;

        cfg_env.handler_cfg.spec_id = spec_id;
    }

    fn fill_tx_env(&self, tx_env: &mut TxEnv, transaction: &TransactionSigned, sender: Address) {
        EthEvmConfig::default().fill_tx_env(tx_env, transaction, sender)
    }

    fn fill_tx_env_system_contract_call(
        &self,
        env: &mut Env,
        caller: Address,
        contract: Address,
        data: Bytes,
    ) {
        EthEvmConfig::default().fill_tx_env_system_contract_call(env, caller, contract, data)
    }
}

impl ConfigureEvm for ExpressEvmConfig {
    type DefaultExternalContext<'a> = ();

    fn evm<DB: Database>(&self, db: DB) -> Evm<'_, Self::DefaultExternalContext<'_>, DB> {
        EvmBuilder::default()
            .with_db(db)
            // add additional precompiles
            .append_handler_register(set_evm_handles)
            .build()
    }

    fn evm_with_inspector<DB, I>(&self, db: DB, inspector: I) -> Evm<'_, I, DB>
    where
        DB: Database,
        I: GetInspector<DB>,
    {
        EvmBuilder::default()
            .with_db(db)
            .with_external_context(inspector)
            // add additional precompiles
            .append_handler_register(set_evm_handles)
            .append_handler_register(inspector_handle_register)
            .build()
    }

    fn default_external_context<'a>(&self) -> Self::DefaultExternalContext<'a> {}
}

/// Add rollup specific customizations to EVM
pub fn set_evm_handles<EXT, DB>(handler: &mut EvmHandler<EXT, DB>)
where
    DB: Database,
{
    let spec_id = handler.cfg.spec_id;

    // install the precompiles
    let prev_handle = handler.pre_execution.load_precompiles.clone();
    handler.pre_execution.load_precompiles = Arc::new(move || {
        let mut precompiles = prev_handle();
        precompiles.extend([(
            crate::precompiles::bridge::BRIDGEOUT_ADDRESS,
            ContextPrecompile::ContextStateful(Arc::new(
                crate::precompiles::bridge::BridgeoutPrecompile::new(FIXED_WITHDRAWAL_WEI),
            )),
        )]);
        precompiles
    });

    // install hook to collect burned gas fees
    let prev_handle = handler.post_execution.reward_beneficiary.clone();
    handler.post_execution.reward_beneficiary = Arc::new(move |context, gas| {
        // Collect "burned" base fee
        if spec_id.is_enabled_in(SpecId::LONDON) {
            let gas_used = U256::from(gas.spent()) - U256::from(gas.refunded());
            let base_fee_rate = context.evm.env.block.basefee;
            let base_fee = gas_used * base_fee_rate;
            update_account_balance(context, BASEFEE_ADDRESS, BalanceUpdate::Add(base_fee))?;
        }

        prev_handle(context, gas)
    })
}

enum BalanceUpdate {
    Add(U256),
    // Sub(U256),
}

fn update_account_balance<EXT, DB: Database>(
    context: &mut Context<EXT, DB>,
    address: Address,
    update: BalanceUpdate,
) -> Result<(), EVMError<DB::Error>> {
    let (account, _) = context.evm.load_account(address)?;

    let balance = account.info.balance;
    let new_balance = match update {
        BalanceUpdate::Add(amount) => balance.saturating_add(amount),
    };

    account.info.balance = new_balance;
    context.evm.journaled_state.touch(&address);

    Ok(())
}
