use std::sync::Arc;

use revm::{handler::register::EvmHandler, Context, ContextPrecompile, Database};
use revm_primitives::{Address, EVMError, SpecId, U256};

use crate::constants::{BASEFEE_ADDRESS, FIXED_WITHDRAWAL_WEI};

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
