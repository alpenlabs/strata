use std::fmt::Display;

use alloy::primitives::{Address as StrataAddress, TxHash};
use bdk_wallet::bitcoin;

pub enum OnchainObject<'a> {
    Transaction(Txid<'a>),
    Address(Address<'a>),
}

pub trait PrettyPrint {
    fn pretty(&self) -> String;
}

impl<'a> OnchainObject<'a> {
    pub fn with_explorer<'b>(self, explorer_domain: &'b str) -> Link<'a, 'b> {
        Link {
            object: self,
            explorer_domain,
        }
    }

    pub fn with_maybe_explorer<'b>(self, explorer_domain: Option<&'b str>) -> MaybeLink<'a, 'b> {
        match explorer_domain {
            Some(dmn) => MaybeLink::Link(self.with_explorer(dmn)),
            None => MaybeLink::Object(self),
        }
    }
}

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

pub struct Link<'a, 'b> {
    /// Object of the link (Transaction or Address)
    object: OnchainObject<'a>,
    /// Domain of the explorer (will be used to build the URL)
    explorer_domain: &'b str,
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

impl<'b, 'a> Display for Link<'a, 'b> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.object {
            OnchainObject::Transaction(ref txid) => match txid {
                Txid::Bitcoin(txid) => write!(f, "https://{}/tx/{}", self.explorer_domain, txid),
                Txid::Strata(txid) => write!(f, "https://{}/tx/{}", self.explorer_domain, txid),
            },
            OnchainObject::Address(ref address) => match address {
                Address::Bitcoin(address) => {
                    write!(f, "https://{}/address/{}", self.explorer_domain, address)
                }
                Address::Strata(address) => {
                    write!(f, "https://{}/address/{}", self.explorer_domain, address)
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
