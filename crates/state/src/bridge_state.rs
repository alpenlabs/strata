//! Bridge state types.
//!
//! This just implements a very simple n-of-n multisig bridge.  It will be
//! extended to a more sophisticated design when we have that specced out.

use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::{
    buf::Buf32,
    l1::{self, OutputRef},
};

/// Global operator idx.
pub type OperatorIdx = u32;

/// Entry for an operator.  They have a
#[derive(Clone, Debug, Eq, PartialEq, Hash, BorshDeserialize, BorshSerialize)]
pub struct OperatorEntry {
    /// Global operator index.
    idx: OperatorIdx,

    /// Pubkey used to verify signed messages from the operator.
    signing_pk: Buf32,

    /// Wallet pubkey used to compute MuSig2 pubkey from a set of operators.
    wallet_pk: Buf32,
}

#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct OperatorTable {
    /// Next unassigned operator index.
    next_idx: u32,

    /// Operator table.
    ///
    /// MUST be sorted by `idx`.
    operators: Vec<OperatorEntry>,
}

impl OperatorTable {
    pub fn new_empty() -> Self {
        Self {
            next_idx: 0,
            operators: Vec::new(),
        }
    }

    /// Sanity checks the operator table for sensibility.
    fn sanity_check(&self) {
        if !self.operators.is_sorted_by_key(|e| e.idx) {
            panic!("bridge_state: operators list not sorted");
        }

        if let Some(e) = self.operators.last() {
            if self.next_idx <= e.idx {
                panic!("bridge_state: operators next_idx before last entry");
            }
        }
    }

    /// Gets an operator from the table by its idx.
    ///
    /// Does a binary search.
    pub fn get_operator(&self, idx: u32) -> Option<&OperatorEntry> {
        self.operators
            .binary_search_by_key(&idx, |e| e.idx)
            .ok()
            .map(|i| &self.operators[i])
    }
}

#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct DepositsTable {
    /// Next unassigned deposit index.
    next_idx: u32,

    /// Deposit table.
    ///
    /// MUST be sorted by `deposit_idx`.
    deposits: Vec<DepositEntry>,
}

impl DepositsTable {
    pub fn new_empty() -> Self {
        Self {
            next_idx: 0,
            deposits: Vec::new(),
        }
    }

    /// Sanity checks the operator table for sensibility.
    fn sanity_check(&self) {
        if !self.deposits.is_sorted_by_key(|e| e.deposit_idx) {
            panic!("bridge_state: deposits list not sorted");
        }

        if let Some(e) = self.deposits.last() {
            if self.next_idx <= e.deposit_idx {
                panic!("bridge_state: deposits next_idx before last entry");
            }
        }
    }

    /// Gets a deposit from the table by its idx.
    ///
    /// Does a binary search.
    pub fn get_deposit(&self, idx: u32) -> Option<&DepositEntry> {
        self.deposits
            .binary_search_by_key(&idx, |e| e.deposit_idx)
            .ok()
            .map(|i| &self.deposits[i])
    }

    /// Gets a mut ref to a deposit from the table by its idx.
    ///
    /// Does a binary search.
    pub fn get_deposit_mut(&mut self, idx: u32) -> Option<&mut DepositEntry> {
        self.deposits
            .binary_search_by_key(&idx, |e| e.deposit_idx)
            .ok()
            .map(|i| &mut self.deposits[i])
    }
}

/// Container for the state machine of a deposit factory.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct DepositEntry {
    deposit_idx: u32,

    /// List of notary operators, by their indexes.
    // TODO convert this to a windowed bitmap or something
    notary_operators: Vec<u32>,

    /// Deposit amount, in the native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Refs to txs in the maturation queue that will update the deposit entry
    /// when they mature.  This is here so that we don't have to scan a
    /// potentially very large set of pending transactions to reason about the
    /// state of the deposits.  This must be kept in sync when we do things
    /// though.
    pending_update_txs: Vec<l1::L1TxRef>,

    /// Deposit state.
    state: DepositState,
}

impl DepositEntry {
    pub fn next_pending_update_tx(&self) -> Option<&l1::L1TxRef> {
        self.pending_update_txs.first()
    }

    pub fn pop_next_pending_deposit(&mut self) -> Option<l1::L1TxRef> {
        if !self.pending_update_txs.is_empty() {
            Some(self.pending_update_txs.remove(0))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub enum DepositState {
    /// Deposit utxo has been recognized.
    Created(CreatedState),

    /// Deposit utxo has been accepted.
    Accepted(AcceptedState),

    /// Order to send out withdrawal dispatched.
    Dispatched(DispatchedState),

    /// Executed state, will be cleaned up.
    Executed,
}

#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct CreatedState {
    /// Output for the proposed thing.
    request_output: OutputRef,

    /// Destination identifier in EL, probably an encoded address.
    dest_ident: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct AcceptedState {
    /// Output the funds are being stored in at rest.
    output: OutputRef,
}

#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct DispatchedState {
    /// Output the funds are still being stored in at rest.
    output: OutputRef,

    /// Configuration for outputs to be written to.
    cmd: DispatchCommand,
}

/// Command to operator(s) to initiate the withdrawal.  Describes the set of
/// outputs we're trying to withdraw to.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct DispatchCommand {
    withdraw_outputs: Vec<WithdrawOutput>,
}

/// An output constructed from [`crate::bridge_ops::WithdrawalIntent`].
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct WithdrawOutput {
    /// Taproot pubkey.
    dest_addr: Buf32,

    /// Amount in sats.
    amt: u64,
}
