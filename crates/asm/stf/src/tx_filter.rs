use std::collections::HashMap;

use bitcoin::{Transaction, opcodes::all::OP_RETURN, script::Instruction};
use strata_asm_common::SubprotocolId;

/// Attempt to parse the SPS-50 L1 transaction header from the first output of a Bitcoin
/// `Transaction`.
///
/// The SPS-50 header MUST be encoded as an `OP_RETURN` in output index 0, with payload:
/// ```text
/// [0..4]   ASCII magic “ALPN”
/// [4]      subprotocol type (u8)
/// [5]      tx type (u8)
/// [6..]    auxiliary data (ignored here)
/// ```
///
/// # Parameters
/// - `tx`: the Bitcoin `Transaction` to inspect.
///
/// # Returns
/// - `Some((subprotocol_type, tx_type))` if:
///   - there is an output at index 0;
///   - its script is `OP_RETURN` with at least 6 bytes of payload;
///   - the first four bytes of the payload equal `b"ALPN"`.
/// - `None` otherwise.
///
/// # Examples
/// ```
/// let (subp, tx_t) = parse_sps50_header(&tx).unwrap();
/// assert_eq!(subp, 1);
/// assert_eq!(tx_t, 7);
/// ```
fn parse_sps50_header(tx: &Transaction) -> Option<(SubprotocolId, u8)> {
    // 1) Ensure there's an output 0
    let first_out = tx.output.first()?;
    let script = &first_out.script_pubkey;

    // 2) Iterate instructions: expect first to be the OP_RETURN opcode
    let mut instrs = script.instructions();
    match instrs.next()? {
        Ok(Instruction::Op(op)) if op == OP_RETURN => {}
        _ => return None,
    }

    // 3) Next instruction must push the header bytes (>= 6 bytes)
    let data = match instrs.next()? {
        Ok(Instruction::PushBytes(d)) if d.len() >= 6 => d,
        _ => return None,
    };

    // 4) Verify magic "ALPN"
    if !data.as_bytes().starts_with(b"ALPN") {
        return None;
    }

    // 4) Extract subprotocol and tx type
    let subprotocol = data[4];
    let tx_type = data[5];
    Some((subprotocol, tx_type))
}

/// Groups only those Bitcoin `Transaction`s tagged with an SPS-50 header,
/// keyed by their subprotocol type.
///
/// Transactions that lack a valid SPS-50 header (wrong magic, not OP_RETURN in output[0],
/// or too-short payload) are filtered out.
pub(crate) fn group_txs_by_subprotocol<T>(
    transactions: T,
) -> HashMap<SubprotocolId, Vec<Transaction>>
where
    T: IntoIterator<Item = Transaction>,
{
    let mut map: HashMap<SubprotocolId, Vec<Transaction>> = HashMap::new();

    for tx in transactions {
        if let Some((subp, _tx_type)) = parse_sps50_header(&tx) {
            map.entry(subp).or_default().push(tx);
        }
    }

    map
}
