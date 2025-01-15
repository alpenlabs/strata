use anyhow::anyhow;
use bitcoin::{
    opcodes::all::OP_PUSHNUM_1,
    script::{Instruction, Instructions},
    secp256k1::{PublicKey, SECP256K1},
    taproot::TaprootBuilder,
    Address, Network, Opcode, XOnlyPublicKey,
};
use musig2::KeyAggContext;
use strata_primitives::{
    buf::Buf32,
    l1::BitcoinAddress,
    params::{OperatorConfig, RollupParams},
};

/// Extract next instruction and try to parse it as an opcode
pub fn next_op(instructions: &mut Instructions<'_>) -> Option<Opcode> {
    let nxt = instructions.next();
    match nxt {
        Some(Ok(Instruction::Op(op))) => Some(op),
        _ => None,
    }
}

/// Extract next instruction and try to parse it as a byte slice
pub fn next_bytes<'a>(instructions: &mut Instructions<'a>) -> Option<&'a [u8]> {
    match instructions.next() {
        Some(Ok(Instruction::PushBytes(bytes))) => Some(bytes.as_bytes()),
        _ => None,
    }
}

/// Extract next integer value(unsigned)
pub fn next_int(instructions: &mut Instructions<'_>) -> Option<u32> {
    let n = instructions.next();
    match n {
        Some(Ok(Instruction::PushBytes(bytes))) => {
            // Convert the bytes to an integer
            if bytes.len() > 4 {
                return None;
            }
            let mut buf = [0; 4];
            buf[..bytes.len()].copy_from_slice(bytes.as_bytes());
            Some(u32::from_le_bytes(buf))
        }
        Some(Ok(Instruction::Op(op))) => {
            // Handle small integers pushed by OP_1 to OP_16
            let opval = op.to_u8();
            let diff = opval - OP_PUSHNUM_1.to_u8();
            if (0..16).contains(&diff) {
                Some(diff as u32 + 1)
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn generate_taproot_address(
    operator_wallet_pks: &[Buf32],
    network: Network,
) -> anyhow::Result<BitcoinAddress> {
    let keys = operator_wallet_pks.iter().map(|op| {
        PublicKey::from_x_only_public_key(
            XOnlyPublicKey::from_slice(op.as_ref()).expect("slice not an x-only public key"),
            bitcoin::key::Parity::Even,
        )
    });

    let x_only_pub_key = KeyAggContext::new(keys)?
        .aggregated_pubkey::<PublicKey>()
        .x_only_public_key()
        .0;

    let taproot_builder = TaprootBuilder::new();
    let spend_info = taproot_builder
        .finalize(SECP256K1, x_only_pub_key)
        .map_err(|_| anyhow!("taproot finalization"))?;
    let merkle_root = spend_info.merkle_root();

    let addr = Address::p2tr(SECP256K1, x_only_pub_key, merkle_root, network);
    let addr = BitcoinAddress::parse(&addr.to_string(), network)?;

    Ok(addr)
}

/// Reads the operator wallet public keys from Rollup params. Returns None if
/// not yet bootstrapped
// FIXME: This is only for devnet as these pks have to be read from the chain state
pub fn get_operator_wallet_pks(params: &RollupParams) -> Vec<Buf32> {
    let OperatorConfig::Static(operator_table) = &params.operator_config;

    operator_table.iter().map(|op| *op.wallet_pk()).collect()
}
