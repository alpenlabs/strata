//! Subprotocol trait definition for ASM.
//!
//! This trait defines the interface every ASM subprotocol implementation must
//! provide. Each subprotocol is responsible for parsing its transactions,
//! updating its internal state, and emitting cross-protocol messages and logs.

use std::any::Any;

use bitcoin::Transaction;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{Log, msg::InterprotoMsg};

/// Identifier for a subprotocol.
pub type SubprotocolId = u8;

/// ASM subprotocol interface.
///
/// A Subprotocol encapsulates a self-contained piece of logic that
///
/// 1. processes each new L1 block to update its own state and emit outgoing inter-protocol
///    messages, and then
/// 2. receives incoming messages to finalize and serialize its state for inclusion in the global
///    AnchorState.
pub trait Subprotocol: 'static {
    /// The subprotocol ID used when searching for relevant transactions.
    const ID: SubprotocolId;

    /// State type serialized into the ASM state structure.
    type State: Any + BorshDeserialize + BorshSerialize;

    /// Message type that we receive messages from other subprotocols using.
    type Msg: Clone + Any;

    /// Constructs a new state to use if the ASM does not have an instance of it.
    fn init() -> Self::State;

    /// Process the transactions and extract all the relevant information from L1 for the
    /// subprotocol
    ///
    /// Update it's own state and output a list of InterProtoMsg addressed to other subprotocols
    fn process_txs(state: &mut Self::State, txs: &[TxInput<'_>], relayer: &mut impl MsgRelayer);

    /// Use the msg other subprotocols to update its state. Also generate the event
    /// logs that is later needed for introspection. Return the commitment of the events. The actual
    /// event is defined by the subprotocol and is not visible to the ASM.
    fn finalize_state(state: &mut Self::State, msgs: &[Self::Msg]);
}

/// Generic message relayer interface.
pub trait MsgRelayer: Any {
    /// Relays a message to the destination subprotocol.
    fn relay_msg(&mut self, m: &dyn InterprotoMsg);

    /// Emits an output log message.
    fn emit_log(&mut self, log: Log);

    /// Gets this msg relayer as a `&dyn Any`.
    fn as_mut_any(&mut self) -> &mut dyn Any;
}

/// A wrapper containing a reference to a Bitcoin `Transaction` together with its
/// parsed SPS-50 payload.
///
/// This struct bundles:
/// 1. `tx`: the original Bitcoin transaction containing the SPS-50 tag in its first output, and
/// 2. `sps_50_payload`: the extracted `Sps50TagPayload`, representing the subprotocol’s transaction
///    type and any auxiliary data.
#[derive(Debug)]
pub struct TxInput<'t> {
    tx: &'t Transaction,
    sps_50_payload: Sps50TagPayload<'t>,
}

/// A parsed SPS-50 tag payload (excluding the “ALPN” magic and subprotocol ID),
/// containing the subprotocol-specific transaction type and any auxiliary data.
///
/// This struct represents everything in the OP_RETURN after the first 6 bytes:
/// 1. Byte 0: subprotocol-defined transaction type
/// 2. Bytes 1…: auxiliary payload (type-specific)
#[derive(Debug)]
pub struct Sps50TagPayload<'p> {
    /// The transaction type as defined by the SPS-50 subprotocol.
    tx_type: u8,

    /// The remaining, type-specific payload for this transaction.
    auxiliary_data: &'p [u8],
}

impl<'p> Sps50TagPayload<'p> {
    /// Constructs a new `Sps50TagPayload`.
    pub fn new(tx_type: u8, auxiliary_data: &'p [u8]) -> Self {
        Self {
            tx_type,
            auxiliary_data,
        }
    }

    /// Returns the subprotocol-defined transaction type.
    pub fn tx_type(&self) -> u8 {
        self.tx_type
    }

    /// Returns the auxiliary data slice associated with this tag.
    pub fn aux_data(&self) -> &[u8] {
        self.auxiliary_data
    }
}

impl<'t> TxInput<'t> {
    /// Create a new `TxInput` referencing the given `Transaction`.
    pub fn new(tx: &'t Transaction, sps_50_info: Sps50TagPayload<'t>) -> Self {
        TxInput {
            tx,
            sps_50_payload: sps_50_info,
        }
    }

    /// Gets the inner transaction.
    pub fn tx(&self) -> &Transaction {
        self.tx
    }

    /// Returns a reference to the parsed SPS-50 payload for this transaction,
    /// which contains the subprotocol-specific transaction type and auxiliary data.
    pub fn sps50_payload(&self) -> &Sps50TagPayload<'t> {
        &self.sps_50_payload
    }
}
