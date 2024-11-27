use std::fmt::Display;

use alloy::primitives::{Address as StrataAddress, TxHash};
use bdk_wallet::bitcoin;

/// A thing that exists or is represented onchain.
/// Currently, this is either a transaction or an address.
pub enum OnchainObject<'a> {
    Transaction(Txid<'a>),
    Address(Address<'a>),
}

impl<'a> From<&'a bitcoin::Address> for OnchainObject<'a> {
    fn from(address: &'a bitcoin::Address) -> Self {
        OnchainObject::Address(Address::Bitcoin(address))
    }
}

impl<'a> From<&'a StrataAddress> for OnchainObject<'a> {
    fn from(address: &'a StrataAddress) -> Self {
        OnchainObject::Address(Address::Strata(address))
    }
}

impl<'a> From<&'a bitcoin::Txid> for OnchainObject<'a> {
    fn from(txid: &'a bitcoin::Txid) -> Self {
        OnchainObject::Transaction(Txid::Bitcoin(txid))
    }
}

impl<'a> From<&'a TxHash> for OnchainObject<'a> {
    fn from(txid: &'a TxHash) -> Self {
        OnchainObject::Transaction(Txid::Strata(txid))
    }
}

impl<'a> OnchainObject<'a> {
    /// Create a link to the object on the given explorer.
    /// Should be of the form `http{s}://{domain}`.
    pub fn with_explorer<'b>(self, explorer: &'b str) -> Link<'a, 'b> {
        Link {
            object: self,
            explorer_ep: explorer,
        }
    }

    /// Create a link to the object on the given explorer, if it exists.
    /// Should be of the form `http{s}://{domain}`.
    ///
    /// If `explorer` is `None`, the object will be represented as-is.
    ///
    /// This is primarily a helper for displaying an OnChainObject in a user-facing context.
    pub fn with_maybe_explorer<'b>(self, explorer: Option<&'b str>) -> MaybeLink<'a, 'b> {
        match explorer {
            Some(dmn) => MaybeLink::Link(self.with_explorer(dmn)),
            None => MaybeLink::Object(self),
        }
    }
}

/// A helper trait for pretty printing something into a human-readable string.
/// Differs from `Display` in that it might add other wording outside of purely
/// the string representation of `self`.
///
/// E.g. a transaction might be prefixed with "Transaction: " or "Tx: ".
pub trait PrettyPrint {
    fn pretty(&self) -> String;
}

/// A helper enum for pretty printing something that might be a link or an object.
pub enum MaybeLink<'a, 'b> {
    Link(Link<'a, 'b>),
    Object(OnchainObject<'a>),
}

impl<'a, 'b> PrettyPrint for MaybeLink<'a, 'b> {
    fn pretty(&self) -> String {
        match self {
            MaybeLink::Link(l) => l.pretty(),
            MaybeLink::Object(o) => o.pretty(),
        }
    }
}

impl PrettyPrint for OnchainObject<'_> {
    fn pretty(&self) -> String {
        match self {
            OnchainObject::Transaction(txid) => format!("Transaction ID: {}", txid),
            OnchainObject::Address(address) => format!("Address: {}", address),
        }
    }
}

impl Display for OnchainObject<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OnchainObject::Transaction(txid) => txid.fmt(f),
            OnchainObject::Address(address) => address.fmt(f),
        }
    }
}

/// A wrapper around a bitcoin transaction id or a strata transaction id.
#[derive(Clone, Copy)]
pub enum Txid<'a> {
    Bitcoin(&'a bitcoin::Txid),
    Strata(&'a TxHash),
}

impl<'a> From<&'a TxHash> for Txid<'a> {
    fn from(v: &'a TxHash) -> Self {
        Self::Strata(v)
    }
}

impl<'a> From<&'a bitcoin::Txid> for Txid<'a> {
    fn from(v: &'a bitcoin::Txid) -> Self {
        Self::Bitcoin(v)
    }
}

impl Display for Txid<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Txid::Bitcoin(txid) => txid.fmt(f),
            Txid::Strata(txid) => txid.fmt(f),
        }
    }
}

/// A wrapper address that wraps either a bitcoin address or a strata address.
#[derive(Clone, Copy)]
pub enum Address<'a> {
    Bitcoin(&'a bitcoin::Address),
    Strata(&'a StrataAddress),
}

impl Display for Address<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Address::Bitcoin(address) => address.fmt(f),
            Address::Strata(address) => address.fmt(f),
        }
    }
}

/// A link to an object on an explorer.
///
/// This is primarily a helper for displaying an OnChainObject in a user-facing
/// context when a explorer URL is known.
pub struct Link<'a, 'b> {
    /// Object of the link (Transaction or Address)
    object: OnchainObject<'a>,
    /// Endpoint of the explorer (will be used to build the URL)
    explorer_ep: &'b str,
}

impl<'b, 'a> Display for Link<'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.object {
            OnchainObject::Transaction(ref txid) => match txid {
                Txid::Bitcoin(txid) => write!(f, "{}/tx/{}", self.explorer_ep, txid),
                Txid::Strata(txid) => write!(f, "{}/tx/{}", self.explorer_ep, txid),
            },
            OnchainObject::Address(ref address) => match address {
                Address::Bitcoin(address) => {
                    write!(f, "{}/address/{}", self.explorer_ep, address)
                }
                Address::Strata(address) => {
                    write!(f, "{}/address/{}", self.explorer_ep, address)
                }
            },
        }
    }
}

impl<'a, 'b> PrettyPrint for Link<'a, 'b> {
    fn pretty(&self) -> String {
        match self.object {
            OnchainObject::Transaction(_) => format!("View transaction at {self}"),
            OnchainObject::Address(_) => format!("View address at {self}"),
        }
    }
}
