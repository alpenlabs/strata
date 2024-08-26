use revm_primitives::{
    address, Address, Bytes, Env, Precompile, PrecompileError, PrecompileErrors, PrecompileOutput,
    PrecompileResult, U256,
};

// TODO: what address to use?
pub const BRIDGEOUT_ADDRESS: Address = address!("000000000000000000000000000000000b121d9e");
const MIN_WITHDRAWAL_WEI: u128 = 1_000_000_000_000_000_000u128;

pub(crate) const BRIDGEOUT: Precompile = Precompile::Env(run);

/// Custom precompile to burn rollup native token and add bridge out intent of equal amount.
/// Bridge out intent is created during block payload generation.
/// This precompile validates transaction and burns the bridge out amount.
fn run(data: &Bytes, _gas: u64, env: &Env) -> PrecompileResult {
    // ensure valid calldata
    if data.len() != 64 {
        return Err(PrecompileErrors::Error(PrecompileError::other(
            "invalid data",
        )));
    }

    // ensure minimum bridgeout amount
    if env.tx.value < U256::from(MIN_WITHDRAWAL_WEI) {
        return Err(PrecompileErrors::Error(PrecompileError::other(
            "below min withdrawal amt",
        )));
    }

    // TODO: burn value

    // TODO: gas for bridge out, using 0 gas currently
    Ok(PrecompileOutput::new(0, Bytes::new()))
}

#[cfg(test)]
mod tests {
    use revm_primitives::TxEnv;

    use super::*;

    #[test]
    fn test_bridgeout_low_amt() {
        for value in [0, MIN_WITHDRAWAL_WEI / 2, MIN_WITHDRAWAL_WEI - 1] {
            let data = [0; 64].into();
            let gas = 5000;
            let env: Env = Env {
                tx: TxEnv {
                    value: U256::from(value),
                    ..Default::default()
                },
                ..Default::default()
            };

            let output = run(&data, gas, &env);
            assert!(matches!(
                output,
                Err(PrecompileErrors::Error(PrecompileError::Other(..)))
            ));
        }
    }

    #[test]
    fn test_bridgeout_invalid_data() {
        for data in [[0; 63].into(), [0; 65].into()] {
            let gas = 5000;
            let env: Env = Env {
                tx: TxEnv {
                    value: U256::from(MIN_WITHDRAWAL_WEI),
                    ..Default::default()
                },
                ..Default::default()
            };

            let output = run(&data, gas, &env);
            assert!(matches!(
                output,
                Err(PrecompileErrors::Error(PrecompileError::Other(..)))
            ));
        }
    }

    #[test]
    fn test_bridgeout_valid() {
        for value in [
            MIN_WITHDRAWAL_WEI,
            MIN_WITHDRAWAL_WEI + 1,
            MIN_WITHDRAWAL_WEI * 1000,
        ] {
            let data = [0; 64].into();
            let gas = 5000;
            let env: Env = Env {
                tx: TxEnv {
                    value: U256::from(value),
                    ..Default::default()
                },
                ..Default::default()
            };

            let output = run(&data, gas, &env);
            assert!(matches!(output, Ok(..)));
        }
    }
}
