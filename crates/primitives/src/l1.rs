use std::{
    fmt, io,
    io::{Read, Write},
    iter::Sum,
    ops::Add,
    str::FromStr,
};

use arbitrary::{Arbitrary, Unstructured};
use bitcoin::{
    address::NetworkUnchecked,
    consensus::serialize,
    hashes::{sha256d, Hash},
    Address, Block, OutPoint,
};
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
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct OutputRef(OutPoint);

impl OutputRef {
    pub fn outpoint(&self) -> &OutPoint {
        &self.0
    }
}

// Implement BorshSerialize for the OutputRef wrapper.
impl BorshSerialize for OutputRef {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        // Serialize the transaction ID as bytes
        writer.write_all(&self.0.txid[..])?;

        // Serialize the output index as a little-endian 4-byte integer
        writer.write_all(&self.0.vout.to_le_bytes())?;
        Ok(())
    }
}

// Implement BorshDeserialize for the OutputRef wrapper.
impl BorshDeserialize for OutputRef {
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self, io::Error> {
        // Read 32 bytes for the transaction ID
        let mut txid_bytes = [0u8; 32];
        reader.read_exact(&mut txid_bytes)?;
        let txid = bitcoin::Txid::from_slice(&txid_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid Txid"))?;

        // Read 4 bytes for the output index
        let mut vout_bytes = [0u8; 4];
        reader.read_exact(&mut vout_bytes)?;
        let vout = u32::from_le_bytes(vout_bytes);

        Ok(OutputRef(OutPoint { txid, vout }))
    }
}

// Implement Arbitrary for the wrapper
impl<'a> Arbitrary<'a> for OutputRef {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Generate a random 32-byte array for the transaction ID (txid)
        let mut txid_bytes = [0u8; 32];
        u.fill_buffer(&mut txid_bytes)?;
        let txid_bytes = &txid_bytes[..];
        let hash = sha256d::Hash::from_slice(txid_bytes).unwrap();
        let txid = bitcoin::Txid::from_slice(&hash[..]).unwrap();

        // Generate a random 4-byte integer for the output index (vout)
        let vout = u.int_in_range(0..=u32::MAX)?;

        Ok(OutputRef(OutPoint { txid, vout }))
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

impl TryFrom<Buf32> for BitcoinAddress {
    type Error = <Address<NetworkUnchecked> as FromStr>::Err;

    fn try_from(value: Buf32) -> Result<Self, Self::Error> {
        let address_str = format!("{:x}", value.0);

        BitcoinAddress::from_str(&address_str)
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
        BitcoinAddress::from_str(s).map_err(serde::de::Error::custom)
    }
}

impl BorshSerialize for BitcoinAddress {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        let addr_str =
            serde_json::to_string(&self).map_err(|e| io::Error::new(ErrorKind::Other, e))?;

        // address serialization adds `"` to both ends of the string (for JSON compatibility)
        let addr_str = addr_str.trim_matches('"');

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
/// NOTE: This wrapper has been created so that we can implement `Borsh*` traits on it.
#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    BorshSerialize,
    BorshDeserialize,
    Arbitrary,
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

impl Add for BitcoinAmount {
    type Output = BitcoinAmount;

    fn add(self, rhs: Self) -> Self::Output {
        Self::from_sat(self.to_sat() + rhs.to_sat())
    }
}

impl Sum for BitcoinAmount {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        Self::from_sat(iter.map(|amt| amt.to_sat()).sum())
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::{hashes::Hash, Txid};

    use super::{BitcoinAddress, BorshDeserialize, BorshSerialize, OutPoint, OutputRef};

    #[test]
    fn borsh_serialization_of_outputref_works() {
        let txid = Txid::from_slice(&[1u8; 32]).unwrap();
        let outpoint = OutPoint { txid, vout: 42 };

        let output_ref = OutputRef(outpoint);

        let mut buffer: Vec<u8> = vec![];
        assert!(
            output_ref.serialize(&mut buffer).is_ok(),
            "borsh serialization of OutputRef failed"
        );

        let de_output_ref = OutputRef::try_from_slice(&buffer[..]);
        assert!(
            de_output_ref.is_ok(),
            "borsh deserialization of OutputRef failed"
        );

        let de_output_ref = de_output_ref.unwrap();

        assert_eq!(
            output_ref, de_output_ref,
            "original and deserialized OutputRefs must be the same"
        );
    }

    #[test]
    fn json_serialization_of_bitcoin_address_works() {
        // this is a random address
        // TODO: implement `Arbitrary` on `BitcoinAddress` and remove this hardcoded value
        let mainnet_addr = "\"bc1qpaj2e2ccwqvyzvsfhcyktulrjkkd28fg75wjuc\"";

        let deserialized_bitcoin_addr: BitcoinAddress =
            serde_json::from_str(mainnet_addr).expect("deserialization of bitcoin address");

        let serialized_bitcoin_addr = serde_json::to_string(&deserialized_bitcoin_addr);

        assert!(
            serialized_bitcoin_addr.is_ok(),
            "serialization of BitcoinAddress must work"
        );

        assert_eq!(
            mainnet_addr,
            serialized_bitcoin_addr.unwrap(),
            "original and serialized addresses must be the same"
        );
    }

    #[test]
    fn borsh_serialization_of_bitcoin_address_works() {
        let mainnet_addr = "bc1qpaj2e2ccwqvyzvsfhcyktulrjkkd28fg75wjuc";

        let addr_bytes = mainnet_addr.as_bytes();

        let deserialized_addr = BitcoinAddress::try_from_slice(addr_bytes);

        assert!(
            deserialized_addr.is_ok(),
            "borsh deserialization of bitcoin address must work"
        );

        let mut serialized_addr: Vec<u8> = vec![];
        deserialized_addr
            .unwrap()
            .serialize(&mut serialized_addr)
            .expect("borsh serialization of bitcoin address must work");

        assert_eq!(
            addr_bytes,
            &serialized_addr[..],
            "original address bytes and serialized address bytes must be the same",
        );
    }
}
