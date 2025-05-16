use bitcoin_bosd::Descriptor;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::l1::BitcoinAmount;

/// Describes an intent to withdraw funds outside of Strata
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct WithdrawalIntent {
    /// Quantity of L1 asset, for Bitcoin this is sats.
    amt: BitcoinAmount,

    /// Destination [`Descriptor`] for the withdrawal
    destination: Descriptor,
}

/// Describes all the messages that can originate when processing ProtocolOps by a subprotocol that
/// needs to be consumed by other subprotocols
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub enum InterProtoMsg {
    /// This message originates from the Core OL Subprotocol and is meant to be passed to the
    /// Bridge Subprotocol
    Withdrawal(WithdrawalIntent),
}
