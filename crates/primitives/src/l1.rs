use std::{
    fmt::Display,
    io::{self, ErrorKind, Read, Write},
    iter::Sum,
    ops::Add,
    str::FromStr,
};

use arbitrary::{Arbitrary, Unstructured};
use bitcoin::{
    absolute::LockTime,
    address::NetworkUnchecked,
    consensus::serialize,
    hashes::{sha256d, Hash},
    key::{rand, Keypair, Parity, TapTweak},
    secp256k1::{SecretKey, XOnlyPublicKey, SECP256K1},
    taproot::{ControlBlock, TaprootMerkleBranch},
    transaction::Version,
    Address, AddressType, Amount, Block, Network, OutPoint, Psbt, ScriptBuf, Sequence, TapNodeHash,
    Transaction, TxIn, TxOut, Txid, Witness,
};
use borsh::{BorshDeserialize, BorshSerialize};
use reth_primitives::revm_primitives::FixedBytes;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::{buf::Buf32, errors::BridgeParseError};

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

impl From<OutPoint> for OutputRef {
    fn from(value: OutPoint) -> Self {
        Self(value)
    }
}

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

impl From<Address<NetworkUnchecked>> for BitcoinAddress {
    fn from(value: Address<NetworkUnchecked>) -> Self {
        Self(value)
    }
}

impl From<Address> for BitcoinAddress {
    fn from(value: Address) -> Self {
        Self(value.as_unchecked().clone())
    }
}

impl BitcoinAddress {
    pub fn new(address: Address<NetworkUnchecked>) -> Self {
        Self(address)
    }

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

impl Display for BitcoinAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Amount> for BitcoinAmount {
    fn from(value: Amount) -> Self {
        Self::from_sat(value.to_sat())
    }
}

impl From<BitcoinAmount> for Amount {
    fn from(value: BitcoinAmount) -> Self {
        Self::from_sat(value.to_sat())
    }
}

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
    /// Serialized length of a u64.
    pub const SIZE: usize = 8;

    /// The number of sats in 1 bitcoin.
    const SATS_FACTOR: u64 = 100_000_000;

    /// Get the number of sats in this [`BitcoinAmount`].
    pub const fn to_sat(&self) -> u64 {
        self.0
    }

    /// Create a [`BitcoinAmount`] with sats precision and the given number of sats.
    pub const fn from_sat(value: u64) -> Self {
        Self(value)
    }

    /// Convert from a value expressing integer values of bitcoins to a [`BitcoinAmount`]
    /// in const context.
    ///
    /// ## Panics
    ///
    /// The function panics if the argument multiplied by the number of sats
    /// per bitcoin overflows a u64 type, or is greater than [`BitcoinAmount::MAX_MONEY`].
    pub const fn from_int_btc(btc: u64) -> Self {
        match btc.checked_mul(Self::SATS_FACTOR) {
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

/// A wrapper around [`Buf32`] for XOnly Schnorr taproot pubkeys.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize,
)]
pub struct XOnlyPk(Buf32);

impl XOnlyPk {
    /// Construct a new [`XOnlyPk`] directly from a [`Buf32`].
    pub fn new(val: Buf32) -> Self {
        Self(val)
    }

    /// Get the underlying [`Buf32`].
    pub fn buf32(&self) -> &Buf32 {
        &self.0
    }

    /// Convert a [`BitcoinAddress`] into a [`XOnlyPk`].
    pub fn from_address(
        address: &BitcoinAddress,
        network: Network,
    ) -> Result<Self, BridgeParseError> {
        let unchecked_addr = address.address().clone();
        let checked_addr = unchecked_addr.require_network(network)?;

        if let Some(AddressType::P2tr) = checked_addr.address_type() {
            let script_pubkey = checked_addr.script_pubkey();

            // skip the version and length bytes
            let pubkey_bytes = &script_pubkey.as_bytes()[2..34];
            let output_key: XOnlyPublicKey = XOnlyPublicKey::from_slice(pubkey_bytes)?;

            let serialized_key: FixedBytes<32> = output_key.serialize().into();

            Ok(Self(Buf32(serialized_key)))
        } else {
            Err(BridgeParseError::UnsupportedAddress)
        }
    }

    /// Convert the [`XOnlyPk`] to an [`Address`].
    pub fn to_address(&self, network: Network) -> Result<Address, BridgeParseError> {
        let buf: [u8; 32] = self.0 .0 .0;
        let pubkey = XOnlyPublicKey::from_slice(&buf)?;

        Ok(Address::p2tr_tweaked(
            pubkey.dangerous_assume_tweaked(),
            network,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BitcoinPsbt(Psbt);

impl BitcoinPsbt {
    pub fn inner(&self) -> &bitcoin::Psbt {
        &self.0
    }

    pub fn compute_txid(&self) -> Txid {
        self.0.unsigned_tx.compute_txid()
    }
}

impl From<Psbt> for BitcoinPsbt {
    fn from(value: bitcoin::Psbt) -> Self {
        Self(value)
    }
}

impl From<BitcoinPsbt> for Psbt {
    fn from(value: BitcoinPsbt) -> Self {
        value.0
    }
}

impl BorshSerialize for BitcoinPsbt {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Serialize the PSBT using bitcoin's built-in serialization
        let psbt_bytes = self.0.serialize();
        // First, write the length of the serialized PSBT (as u64)
        BorshSerialize::serialize(&(psbt_bytes.len() as u64), writer)?;
        // Then, write the actual serialized PSBT bytes
        writer.write_all(&psbt_bytes)?;
        Ok(())
    }
}

impl BorshDeserialize for BitcoinPsbt {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        // First, read the length of the PSBT (as u64)
        let len = u64::deserialize_reader(reader)? as usize;
        // Then, create a buffer to hold the PSBT bytes and read them
        let mut psbt_bytes = vec![0u8; len];
        reader.read_exact(&mut psbt_bytes)?;
        // Use the bitcoin crate's deserialize method to create a Psbt from the bytes
        let psbt = Psbt::deserialize(&psbt_bytes).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid PSBT data")
        })?;
        Ok(BitcoinPsbt(psbt))
    }
}

impl<'a> Arbitrary<'a> for BitcoinPsbt {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let num_outputs = u.arbitrary_len::<[u8; 32]>()? % 5;
        let mut output: Vec<TxOut> = vec![];

        for _ in 0..num_outputs {
            let txout = BitcoinTxOut::arbitrary(u)?;
            let txout = TxOut::from(txout);

            output.push(txout);
        }

        let tx = Transaction {
            version: Version(1),
            lock_time: LockTime::from_consensus(0),
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                witness: Witness::new(),
                sequence: Sequence(0),
                script_sig: ScriptBuf::new(),
            }],
            output,
        };

        let psbt = Psbt::from_unsigned_tx(tx).map_err(|_e| arbitrary::Error::IncorrectFormat)?;
        let psbt = BitcoinPsbt::from(psbt);

        Ok(psbt)
    }
}

/// A wrapper around [`bitcoin::TxOut`] that implements some additional traits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BitcoinTxOut(bitcoin::TxOut);

impl BitcoinTxOut {
    pub fn inner(&self) -> &bitcoin::TxOut {
        &self.0
    }
}

impl From<bitcoin::TxOut> for BitcoinTxOut {
    fn from(value: bitcoin::TxOut) -> Self {
        Self(value)
    }
}

impl From<BitcoinTxOut> for bitcoin::TxOut {
    fn from(value: BitcoinTxOut) -> Self {
        value.0
    }
}

// Implement BorshSerialize for BitcoinTxOut
impl BorshSerialize for BitcoinTxOut {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Serialize the value (u64)
        BorshSerialize::serialize(&self.0.value.to_sat(), writer)?;

        // Serialize the script_pubkey (ScriptBuf)
        let script_bytes = self.0.script_pubkey.to_bytes();
        BorshSerialize::serialize(&(script_bytes.len() as u64), writer)?;
        writer.write_all(&script_bytes)?;

        Ok(())
    }
}

// Implement BorshDeserialize for BitcoinTxOut
impl BorshDeserialize for BitcoinTxOut {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        // Deserialize the value (u64)
        let value = u64::deserialize_reader(reader)?;

        // Deserialize the script_pubkey (ScriptBuf)
        let script_len = u64::deserialize_reader(reader)? as usize;
        let mut script_bytes = vec![0u8; script_len];
        reader.read_exact(&mut script_bytes)?;
        let script_pubkey = ScriptBuf::from(script_bytes);

        Ok(BitcoinTxOut(bitcoin::TxOut {
            value: Amount::from_sat(value),
            script_pubkey,
        }))
    }
}

/// Implement Arbitrary for ArbitraryTxOut
impl<'a> Arbitrary<'a> for BitcoinTxOut {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Generate arbitrary value and script for the TxOut
        let value = u64::arbitrary(u)?;
        let script_len = usize::arbitrary(u)? % 100; // Limit script length
        let script_bytes = u.bytes(script_len)?;
        let script_pubkey = bitcoin::ScriptBuf::from(script_bytes.to_vec());

        Ok(Self(TxOut {
            value: Amount::from_sat(value),
            script_pubkey,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpendInfo {
    pub script_buf: ScriptBuf,
    pub control_block: ControlBlock,
}

// Implement BorshSerialize for SpendInfo
impl BorshSerialize for SpendInfo {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Serialize the ScriptBuf
        let script_bytes = self.script_buf.to_bytes();
        BorshSerialize::serialize(&(script_bytes.len() as u64), writer)?;
        writer.write_all(&script_bytes)?;

        // Serialize the ControlBlock using bitcoin's serialize method
        let control_block_bytes = self.control_block.serialize();
        BorshSerialize::serialize(&(control_block_bytes.len() as u64), writer)?;
        writer.write_all(&control_block_bytes)?;

        Ok(())
    }
}

// Implement BorshDeserialize for SpendInfo
impl BorshDeserialize for SpendInfo {
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        // Deserialize the ScriptBuf
        let script_len = u64::deserialize_reader(reader)? as usize;
        let mut script_bytes = vec![0u8; script_len];
        reader.read_exact(&mut script_bytes)?;
        let script_buf = ScriptBuf::from(script_bytes);

        // Deserialize the ControlBlock
        let control_block_len = u64::deserialize_reader(reader)? as usize;
        let mut control_block_bytes = vec![0u8; control_block_len];
        reader.read_exact(&mut control_block_bytes)?;
        let control_block: ControlBlock =
            ControlBlock::decode(&control_block_bytes[..]).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid ControlBlock")
            })?;

        Ok(SpendInfo {
            script_buf,
            control_block,
        })
    }
}

impl<'a> Arbitrary<'a> for SpendInfo {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Arbitrary ScriptBuf (the script part of SpendInfo)
        let script_len = usize::arbitrary(u)? % 100; // Limit the length of the script for practicality
        let script_bytes = u.bytes(script_len)?; // Generate random bytes for the script
        let script_buf = ScriptBuf::from(script_bytes.to_vec());

        // Now we will manually generate the fields of the ControlBlock struct

        // Arbitrary leaf version
        let leaf_version = bitcoin::taproot::LeafVersion::TapScript;

        // Arbitrary output key parity (Even or Odd)
        let output_key_parity = if bool::arbitrary(u)? {
            Parity::Even
        } else {
            Parity::Odd
        };

        let secret_key = SecretKey::new(&mut rand::thread_rng());
        let keypair = Keypair::from_secret_key(SECP256K1, &secret_key);
        let (internal_key, _) = XOnlyPublicKey::from_keypair(&keypair);

        // Arbitrary Taproot merkle branch (vector of 32-byte hashes)
        const BRANCH_LENGTH: usize = 10;
        let mut tapnode_hashes: Vec<TapNodeHash> = Vec::with_capacity(BRANCH_LENGTH);
        for _ in 0..BRANCH_LENGTH {
            let hash = TapNodeHash::from_slice(&<[u8; 32]>::arbitrary(u)?)
                .map_err(|_e| arbitrary::Error::IncorrectFormat)?;
            tapnode_hashes.push(hash);
        }

        let tapnode_hashes: &[TapNodeHash; BRANCH_LENGTH] =
            &tapnode_hashes[..BRANCH_LENGTH].try_into().unwrap();

        let merkle_branch = TaprootMerkleBranch::from(*tapnode_hashes);

        // Construct the ControlBlock manually
        let control_block = ControlBlock {
            leaf_version,
            output_key_parity,
            internal_key,
            merkle_branch,
        };

        // Construct the SpendInfo
        Ok(SpendInfo {
            script_buf,
            control_block,
        })
    }
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use bitcoin::{
        key::{Keypair, Secp256k1},
        opcodes::all::OP_CHECKSIG,
        script::Builder,
        secp256k1::{All, SecretKey},
        taproot::{ControlBlock, TaprootBuilder, TaprootMerkleBranch},
        Address, Amount, Network, ScriptBuf, TapNodeHash, TxOut, XOnlyPublicKey,
    };

    use super::{BitcoinAddress, BitcoinAmount, BorshDeserialize, BorshSerialize, XOnlyPk};
    use crate::l1::{BitcoinPsbt, BitcoinTxOut, SpendInfo};

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

    #[test]
    fn bitcoin_addr_to_taproot_pubkey_conversion_works() {
        let secp = Secp256k1::new();
        let network = Network::Bitcoin;
        let (address, _) = get_taproot_address(&secp, network);

        let taproot_pubkey = XOnlyPk::from_address(&address, network);

        assert!(
            taproot_pubkey.is_ok(),
            "conversion from address to taproot pubkey failed"
        );

        let taproot_pubkey = taproot_pubkey.unwrap();
        let bitcoin_address = taproot_pubkey.to_address(network);

        assert!(
            bitcoin_address.is_ok(),
            "conversion from taproot pubkey to address failed"
        );

        let bitcoin_address = bitcoin_address.unwrap();
        let unchecked_addr = bitcoin_address.as_unchecked();

        let new_taproot_pubkey =
            XOnlyPk::from_address(&BitcoinAddress::new(unchecked_addr.clone()), network);

        assert_eq!(
            unchecked_addr,
            address.address(),
            "converted and original addresses must be the same"
        );

        assert_eq!(
            taproot_pubkey,
            new_taproot_pubkey.unwrap(),
            "converted and original taproot pubkeys must be the same"
        );
    }

    #[test]
    #[should_panic(expected = "number of sats greater than u64::MAX")]
    fn bitcoinamount_should_handle_sats_exceeding_u64_max() {
        let bitcoins: u64 = u64::MAX / BitcoinAmount::SATS_FACTOR + 1;

        BitcoinAmount::from_int_btc(bitcoins);
    }

    fn get_taproot_address(
        secp: &Secp256k1<All>,
        network: Network,
    ) -> (BitcoinAddress, Option<TapNodeHash>) {
        let internal_pubkey = get_random_pubkey_from_slice(secp, &[0x12; 32]);

        let pk1 = get_random_pubkey_from_slice(secp, &[0x02; 32]);

        let mut script1 = ScriptBuf::new();
        script1.push_slice(pk1.serialize());
        script1.push_opcode(OP_CHECKSIG);

        let pk2 = get_random_pubkey_from_slice(secp, &[0x05; 32]);

        let mut script2 = ScriptBuf::new();
        script2.push_slice(pk2.serialize());
        script2.push_opcode(OP_CHECKSIG);

        let taproot_builder = TaprootBuilder::new()
            .add_leaf(1, script1)
            .unwrap()
            .add_leaf(1, script2)
            .unwrap();

        let tree_info = taproot_builder.finalize(secp, internal_pubkey).unwrap();
        let merkle_root = tree_info.merkle_root();

        let taproot_address = Address::p2tr(secp, internal_pubkey, merkle_root, network);

        (
            BitcoinAddress::new(taproot_address.as_unchecked().clone()),
            merkle_root,
        )
    }

    #[test]
    fn test_bitcoinpsbt_serialize_deserialize() {
        // Create an arbitrary PSBT
        let random_data = &[0u8; 1024];
        let mut unstructured = Unstructured::new(&random_data[..]);
        let bitcoin_psbt: BitcoinPsbt = BitcoinPsbt::arbitrary(&mut unstructured).unwrap();

        // Serialize the struct
        let mut serialized = vec![];
        bitcoin_psbt
            .serialize(&mut serialized)
            .expect("Serialization failed");

        // Deserialize the struct
        let deserialized: BitcoinPsbt =
            BitcoinPsbt::deserialize(&mut &serialized[..]).expect("Deserialization failed");

        // Ensure the deserialized PSBT matches the original
        assert_eq!(bitcoin_psbt.0, deserialized.0);
    }

    #[test]
    fn test_spendinfo_serialize_deserialize() {
        // Create a dummy ScriptBuf
        let script_buf = Builder::new()
            .push_opcode(bitcoin::blockdata::opcodes::all::OP_CHECKSIG)
            .into_script();

        // Create a dummy ControlBlock
        let tapnode_hash: [TapNodeHash; 0] = [];
        let control_block = ControlBlock {
            leaf_version: bitcoin::taproot::LeafVersion::TapScript,
            internal_key: get_random_pubkey_from_slice(&Secp256k1::new(), &[0x12; 32]),
            merkle_branch: TaprootMerkleBranch::from(tapnode_hash),
            output_key_parity: bitcoin::key::Parity::Odd,
        };

        let spend_info = SpendInfo {
            script_buf,
            control_block,
        };

        // Serialize the struct
        let mut serialized = vec![];
        spend_info
            .serialize(&mut serialized)
            .expect("Serialization failed");

        // Deserialize the struct
        let deserialized: SpendInfo =
            SpendInfo::deserialize(&mut &serialized[..]).expect("Deserialization failed");

        // Ensure the deserialized SpendInfo matches the original
        assert_eq!(spend_info.script_buf, deserialized.script_buf);
        assert_eq!(spend_info.control_block, deserialized.control_block);
    }

    #[test]
    fn test_bitcointxout_serialize_deserialize() {
        // Create a dummy TxOut with a simple script
        let script = Builder::new()
            .push_opcode(bitcoin::blockdata::opcodes::all::OP_CHECKSIG)
            .into_script();
        let tx_out = TxOut {
            value: Amount::from_sat(1000),
            script_pubkey: script,
        };

        let bitcoin_tx_out = BitcoinTxOut(tx_out);

        // Serialize the BitcoinTxOut struct
        let mut serialized = vec![];
        bitcoin_tx_out
            .serialize(&mut serialized)
            .expect("Serialization failed");

        // Deserialize the BitcoinTxOut struct
        let deserialized: BitcoinTxOut =
            BitcoinTxOut::deserialize(&mut &serialized[..]).expect("Deserialization failed");

        // Ensure the deserialized BitcoinTxOut matches the original
        assert_eq!(bitcoin_tx_out.0.value, deserialized.0.value);
        assert_eq!(bitcoin_tx_out.0.script_pubkey, deserialized.0.script_pubkey);
    }

    fn get_random_pubkey_from_slice(secp: &Secp256k1<All>, buf: &[u8]) -> XOnlyPublicKey {
        let sk = SecretKey::from_slice(buf).unwrap();
        let keypair = Keypair::from_secret_key(secp, &sk);
        let (pk, _) = XOnlyPublicKey::from_keypair(&keypair);

        pk
    }
}
