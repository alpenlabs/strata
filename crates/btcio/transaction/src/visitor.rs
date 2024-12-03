use bitcoin::Transaction;
use strata_state::{
    batch::SignedBatchCheckpoint,
    da_blob::PayloadCommitment,
    tx::{DepositInfo, DepositRequestInfo, ProtocolOperation},
};

use crate::{
    deposit::{deposit_request::extract_deposit_request_info, deposit_tx::extract_deposit_info},
    reveal::parser::parse_reveal_script,
    TxFilterConfig,
};

/// Trait for visiting different types of protocol operations.
pub trait OpVisitor<'t> {
    fn visit_deposit(&mut self, deposit_info: DepositInfo);
    fn visit_checkpoint(&mut self, checkpoint_data: SignedBatchCheckpoint);
    fn visit_envelope_payload_chunks(&mut self, chunks: impl Iterator<Item = &'t [u8]>);
}

/// Trait for visiting transactions to extract protocol-relevant information.
pub trait TxVisitor<'t> {
    fn visit_tx(&mut self, filter_config: &TxFilterConfig, tx: &'t Transaction);
}

/// Struct that delegates transaction visits to an `OpVisitor`.
pub struct OpTxVisitor<'t, V: OpVisitor<'t>> {
    op_visitor: V,
    phantom: std::marker::PhantomData<&'t ()>,
}

impl<'t, V: OpVisitor<'t>> OpTxVisitor<'t, V> {
    /// Creates a new instance of `OpTxVisitor`.
    pub fn new(op_visitor: V) -> Self {
        Self {
            op_visitor,
            phantom: std::marker::PhantomData,
        }
    }

    /// Returns a reference to the inner `OpVisitor`.
    pub fn op_visitor(&self) -> &V {
        &self.op_visitor
    }

    /// Returns a mutable reference to the inner `OpVisitor`.
    pub fn op_visitor_mut(&mut self) -> &mut V {
        &mut self.op_visitor
    }
}

impl<'t, V: OpVisitor<'t>> TxVisitor<'t> for OpTxVisitor<'t, V> {
    fn visit_tx(&mut self, config: &TxFilterConfig, tx: &'t Transaction) {
        // Iterate over transaction inputs and process their witness tapscript.
        tx.input.iter().for_each(|inp| {
            if let Some(scr) = inp.witness.tapscript() {
                let _ = parse_reveal_script(
                    scr,
                    &config.da_tag,
                    &config.ckpt_tag,
                    &mut self.op_visitor,
                );
            }
        });
        // Extract deposit information and delegate to the `OpVisitor`.
        if let Some(deposit_info) = extract_deposit_info(tx, &config.deposit_config) {
            self.op_visitor.visit_deposit(deposit_info);
        }
    }
}

/// Visitor implementation for scanning and collecting protocol operations.
pub struct ScanProofOpVisitor {
    protocol_ops: Vec<ProtocolOperation>,
}

impl ScanProofOpVisitor {
    /// Creates a new instance of `ScanProofOpVisitor`.
    pub fn new() -> Self {
        Self {
            protocol_ops: Vec::new(),
        }
    }

    /// Returns a reference to the collected protocol operations.
    pub fn protocol_ops(&self) -> &[ProtocolOperation] {
        &self.protocol_ops
    }
}

impl Default for ScanProofOpVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl<'t> OpVisitor<'t> for ScanProofOpVisitor {
    fn visit_checkpoint(&mut self, checkpoint_data: SignedBatchCheckpoint) {
        self.protocol_ops
            .push(ProtocolOperation::Checkpoint(checkpoint_data));
    }

    fn visit_envelope_payload_chunks(&mut self, chunks: impl Iterator<Item = &'t [u8]>) {
        let commitment = PayloadCommitment::from_slice(
            &chunks
                .flat_map(|slice| slice.iter().copied())
                .collect::<Vec<_>>(),
        );
        self.protocol_ops.push(ProtocolOperation::DA(commitment));
    }

    fn visit_deposit(&mut self, deposit_info: DepositInfo) {
        self.protocol_ops
            .push(ProtocolOperation::Deposit(deposit_info));
    }
}

/// Visitor implementation for processing deposit requests.
pub struct BridgeVisitor {
    deposit_requests: Vec<DepositRequestInfo>,
}

impl BridgeVisitor {
    /// Creates a new instance of [`BridgeVisitor`].
    pub fn new() -> Self {
        Self {
            deposit_requests: Vec::new(),
        }
    }

    /// Returns a reference to the collected deposit requests.
    pub fn deposit_requests(&self) -> &[DepositRequestInfo] {
        &self.deposit_requests
    }
}

impl Default for BridgeVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl<'t> TxVisitor<'t> for BridgeVisitor {
    fn visit_tx(&mut self, config: &TxFilterConfig, tx: &'t Transaction) {
        if let Some(deposit_request_info) = extract_deposit_request_info(tx, &config.deposit_config)
        {
            self.deposit_requests.push(deposit_request_info)
        }
    }
}
