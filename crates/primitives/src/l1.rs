use std::{
    fmt, io,
    io::{Read, Write},
    str::FromStr,
};

use arbitrary::Arbitrary;
use bitcoin::{address::NetworkUnchecked, consensus::serialize, hashes::Hash, Address, Block};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_json;

use crate::buf::Buf32;

/// Reference to a transaction in a block.  This is the block index and the
/// position of the transaction in the block.
#[derive(
    Copy,
    Clone,
    Debug,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Arbitrary,
    BorshDeserialize,
    BorshSerialize,
)]
pub struct L1TxRef(u64, u32);

impl From<L1TxRef> for (u64, u32) {
    fn from(val: L1TxRef) -> Self {
        (val.0, val.1)
    }
}

impl From<(u64, u32)> for L1TxRef {
    fn from(value: (u64, u32)) -> Self {
        Self(value.0, value.1)
    }
}

/// TODO: This is duplicate with alpen_state::l1::L1TxProof
/// Merkle proof for a TXID within a block.
// TODO rework this, make it possible to generate proofs, etc.
#[derive(Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct L1TxProof {
    position: u32,
    cohashes: Vec<Buf32>,
}

impl L1TxProof {
    pub fn new(position: u32, cohashes: Vec<Buf32>) -> Self {
        Self { position, cohashes }
    }

    pub fn cohashes(&self) -> &[Buf32] {
        &self.cohashes
    }

    pub fn position(&self) -> u32 {
        self.position
    }
}

/// Tx body with a proof.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1Tx {
    proof: L1TxProof,
    tx: Vec<u8>,
}

impl L1Tx {
    pub fn new(proof: L1TxProof, tx: Vec<u8>) -> Self {
        Self { proof, tx }
    }

    pub fn proof(&self) -> &L1TxProof {
        &self.proof
    }

    pub fn tx_data(&self) -> &[u8] {
        &self.tx
    }
}

/// Describes an L1 block and associated data that we need to keep around.
// TODO should we include the block index here?
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1BlockManifest {
    /// Block hash/ID, kept here so we don't have to be aware of the hash function
    /// here.  This is what we use in the MMR.
    blockid: Buf32,

    /// Block header and whatever additional data we might want to query.
    header: Vec<u8>,

    /// Merkle root for the transactions in the block.  For Bitcoin, this is
    /// actually the witness transactions root, since we care about the witness
    /// data.
    txs_root: Buf32,
}

impl L1BlockManifest {
    pub fn new(blockid: Buf32, header: Vec<u8>, txs_root: Buf32) -> Self {
        Self {
            blockid,
            header,
            txs_root,
        }
    }

    pub fn block_hash(&self) -> Buf32 {
        self.blockid
    }

    pub fn header(&self) -> &[u8] {
        &self.header
    }

    /// Witness transactions root.
    pub fn txs_root(&self) -> Buf32 {
        self.txs_root
    }
}

impl From<Block> for L1BlockManifest {
    fn from(block: Block) -> Self {
        let blockid = Buf32(block.block_hash().to_raw_hash().to_byte_array().into());
        let root = block
            .witness_root()
            .map(|x| x.to_byte_array())
            .unwrap_or_default();
        let header = serialize(&block.header);
        Self {
            blockid,
            txs_root: Buf32(root.into()),
            header,
        }
    }
}

/// L1 output reference.
#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Arbitrary, BorshDeserialize, BorshSerialize,
)]
pub struct OutputRef {
    txid: Buf32,
    outidx: u16,
}

impl OutputRef {
    pub fn new(txid: Buf32, outidx: u16) -> Self {
        Self { txid, outidx }
    }

    pub fn txid(&self) -> &Buf32 {
        &self.txid
    }

    pub fn outidx(&self) -> u16 {
        self.outidx
    }
}

impl From<OutputRef> for (Buf32, u16) {
    fn from(val: OutputRef) -> Self {
        (val.txid, val.outidx)
    }
}

impl fmt::Debug for OutputRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:?}:{}", self.txid, self.outidx))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct L1Status {
    /// If the last time we tried to poll the client (as of `last_update`)
    /// we were successful.
    pub bitcoin_rpc_connected: bool,

    /// The last error message we received when trying to poll the client, if
    /// there was one.
    pub last_rpc_error: Option<String>,

    /// Current block height.
    pub cur_height: u64,

    /// Current tip block ID as string.
    pub cur_tip_blkid: String,

    /// Last published txid where L2 blob was present
    pub last_published_txid: Option<String>,

    /// UNIX millis time of the last time we got a new update from the L1 connector.
    pub last_update: u64,
}

/// A wrapper around the [`bitcoin::Address<NetworkUnchecked>`] type created in order to implement
/// some useful traits on it such as [`serde::Deserialize`], [`borsh::BorshSerialize`] and
/// [`borsh::BorshDeserialize`].
// TODO: implement [`arbitrary::Arbitrary`]?
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BitcoinAddress(Address<NetworkUnchecked>);

impl FromStr for BitcoinAddress {
    type Err = <Address<NetworkUnchecked> as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let address = Address::from_str(s)?;

        Ok(Self(address))
    }
}

impl BitcoinAddress {
    pub fn address(&self) -> &Address<NetworkUnchecked> {
        &self.0
    }
}

impl<'de> Deserialize<'de> for BitcoinAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        Address::from_str(s)
            .map(BitcoinAddress)
            .map_err(serde::de::Error::custom)
    }
}

impl BorshSerialize for BitcoinAddress {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        let addr_str = serde_json::to_string(&self)?;
        writer.write_all(addr_str.as_bytes())?;
        Ok(())
    }
}

impl BorshDeserialize for BitcoinAddress {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self, io::Error> {
        let mut addr_bytes = Vec::new();
        reader.read_to_end(&mut addr_bytes)?;
        let addr_str = String::from_utf8(addr_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))?;
        let address = Address::from_str(&addr_str)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid Bitcoin address"))?;
        Ok(BitcoinAddress(address))
    }
}

/// A wrapper for bitcoin amount in sats similar to the implementation in [`bitcoin::Amount`].
///
/// **NOTE**: This wrapper has been created so that we can implement `Borsh*` traits on it.
#[derive(
    Debug, Clone, Serialize, Deserialize, Eq, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary,
)]
pub struct BitcoinAmount(u64);

impl BitcoinAmount {
    // The zero amount.
    pub const ZERO: BitcoinAmount = Self(0);
    /// The maximum value allowed as an amount. Useful for sanity checking.
    pub const MAX_MONEY: BitcoinAmount = Self::from_int_btc(21_000_000);
    /// The minimum value of an amount.
    pub const MIN: BitcoinAmount = Self::ZERO;
    /// The maximum value of an amount.
    pub const MAX: BitcoinAmount = Self(u64::MAX);
    /// The number of bytes that an amount contributes to the size of a transaction.
    pub const SIZE: usize = 8; // Serialized length of a u64.

    /// Get the number of sats in this [`BitcoinAmount`].
    pub fn to_sat(&self) -> u64 {
        self.0
    }

    /// Create a [`BitcoinAmount`] with sats precision and the given number of sats.
    pub const fn from_sat(value: u64) -> Self {
        Self(value)
    }

    /// Convert from a value expressing integer values of bitcoins to an [`BitcoinAmount`]
    /// in const context.
    ///
    /// ## Panics
    ///
    /// The function panics if the argument multiplied by the number of sats
    /// per bitcoin overflows a u64 type.
    pub const fn from_int_btc(btc: u64) -> Self {
        match btc.checked_mul(100_000_000) {
            Some(amount) => Self::from_sat(amount),
            None => {
                panic!("number of sats greater than u64::MAX");
            }
        }
    }
}
