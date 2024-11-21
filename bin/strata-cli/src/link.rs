use std::fmt::Display;

use alloy::primitives::{Address as StrataAddress, TxHash};
use bdk_wallet::bitcoin;

enum OnchainObject<'a> {
    Transaction(Txid<'a>),
    Address(Address<'a>),
}

#[derive(Clone, Copy)]
enum Txid<'a> {
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

impl Display for Address<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Address::Bitcoin(address) => address.fmt(f),
            Address::Strata(address) => address.fmt(f),
        }
    }
}

#[derive(Clone, Copy)]
enum Address<'a> {
    Bitcoin(&'a bitcoin::Address),
    Strata(&'a StrataAddress),
}

pub struct Link<'a, 'b> {
    /// Object of the link (Transaction or Address)
    object: OnchainObject<'a>,
    /// Domain of the explorer (will be used to build the URL)
    explorer_domain: &'b str,
}

impl<'a, 'b> From<(&'a bitcoin::Address, &'b str)> for Link<'a, 'b> {
    fn from((address, explorer_domain): (&'a bitcoin::Address, &'b str)) -> Self {
        Link {
            object: OnchainObject::Address(Address::Bitcoin(address)),
            explorer_domain,
        }
    }
}

impl<'a, 'b> From<(&'a StrataAddress, &'b str)> for Link<'a, 'b> {
    fn from((address, explorer_domain): (&'a StrataAddress, &'b str)) -> Self {
        Link {
            object: OnchainObject::Address(Address::Strata(address)),
            explorer_domain,
        }
    }
}

impl<'a, 'b> From<(&'a bitcoin::Txid, &'b str)> for Link<'a, 'b> {
    fn from((txid, explorer_domain): (&'a bitcoin::Txid, &'b str)) -> Self {
        Link {
            object: OnchainObject::Transaction(Txid::Bitcoin(txid)),
            explorer_domain,
        }
    }
}

impl<'a, 'b> From<(&'a TxHash, &'b str)> for Link<'a, 'b> {
    fn from((txid, explorer_domain): (&'a TxHash, &'b str)) -> Self {
        Link {
            object: OnchainObject::Transaction(Txid::Strata(txid)),
            explorer_domain,
        }
    }
}

impl<'a, 'b> From<(Txid<'a>, &'b str)> for Link<'a, 'b> {
    fn from((txid, explorer): (Txid<'a>, &'b str)) -> Self {
        match txid {
            Txid::Bitcoin(txid) => (txid, explorer).into(),
            Txid::Strata(txid) => (txid, explorer).into(),
        }
    }
}

impl<'a, 'b> From<(Address<'a>, &'b str)> for Link<'a, 'b> {
    fn from((address, explorer_domain): (Address<'a>, &'b str)) -> Self {
        match address {
            Address::Bitcoin(address) => (address, explorer_domain).into(),
            Address::Strata(address) => (address, explorer_domain).into(),
        }
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

pub fn pretty_txid<'a, 'b>(txid: Txid, explorer: &'b Option<String>) -> String {
    match explorer {
        Some(endpoint) => {
            let link = Link::from((txid, endpoint.as_str()));
            format!("Transaction ID {}. View it at {}", txid, link)
        }
        None => {
            format!("Transaction ID {}.", txid)
        }
    }
}
pub fn pretty_address<'a, 'b>(address: Address, explorer: &'b Option<String>) -> String {
    match explorer {
        Some(endpoint) => {
            let link = Link::from((address, endpoint.as_str()));
            format!("Address {}. View it at {}", address, link)
        }
        None => {
            format!("Address {} is associated with the blockchain.", address)
        }
    }
}
