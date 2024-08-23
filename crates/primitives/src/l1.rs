use std::{
    io::{self, ErrorKind, Read, Write},
    iter::Sum,
    ops::{Add, Deref, DerefMut},
    str::FromStr,
};

use anyhow::{anyhow, Context};
use arbitrary::{Arbitrary, Unstructured};
use bitcoin::{
    address::NetworkUnchecked,
    consensus::serialize,
    hashes::{sha256d, Hash},
    key::TapTweak,
    Address, AddressType, Amount, Block, Network, OutPoint, XOnlyPublicKey,
};
use borsh::{BorshDeserialize, BorshSerialize};
use reth_primitives::revm_primitives::FixedBytes;
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

/// A wrapper for [`bitcoin::Amount`].
///
/// NOTE: This wrapper has been created so that we can implement `Borsh*` traits on it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub struct BitcoinAmount(Amount);

impl From<Amount> for BitcoinAmount {
    fn from(value: Amount) -> Self {
        BitcoinAmount(value)
    }
}

impl Deref for BitcoinAmount {
    type Target = Amount;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BitcoinAmount {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BorshSerialize for BitcoinAmount {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let sats = self.0.to_sat();

        borsh::BorshSerialize::serialize(&sats, writer)
    }
}

impl BorshDeserialize for BitcoinAmount {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        let sats = borsh::BorshDeserialize::deserialize(buf)?;

        Ok(BitcoinAmount(Amount::from_sat(sats)))
    }

    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let sats = borsh::BorshDeserialize::deserialize_reader(reader)?;

        Ok(BitcoinAmount(Amount::from_sat(sats)))
    }
}

impl<'a> Arbitrary<'a> for BitcoinAmount {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        // Generate a random u64 value and convert it to Amount
        let sats: u64 = u.arbitrary()?;
        Ok(BitcoinAmount(Amount::from_sat(sats)))
    }
}

impl Add for BitcoinAmount {
    type Output = BitcoinAmount;

    fn add(self, rhs: Self) -> Self::Output {
        BitcoinAmount(self.0 + rhs.0)
    }
}

impl Sum for BitcoinAmount {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let total_amt = iter.fold(Amount::ZERO, |acc, amt| acc + amt.0);

        BitcoinAmount(total_amt)
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
    pub fn from_address(address: &BitcoinAddress, network: Network) -> anyhow::Result<Self> {
        let unchecked_addr = address.address().clone();
        let checked_addr = unchecked_addr.require_network(network)?;

        if let Some(AddressType::P2tr) = checked_addr.address_type() {
            let script_pubkey = checked_addr.script_pubkey();

            // skip the version and length bytes
            let pubkey_bytes = &script_pubkey.as_bytes()[2..34];
            let output_key: XOnlyPublicKey = XOnlyPublicKey::from_slice(pubkey_bytes)
                .context("invalid key format for taproot address")?;

            let serialized_key: FixedBytes<32> = output_key.serialize().into();

            Ok(Self(Buf32(serialized_key)))
        } else {
            Err(anyhow!("Address is not a P2TR (Taproot) address"))
        }
    }

    /// Convert the [`XOnlyPk`] to an [`Address`].
    pub fn to_address(&self, network: Network) -> anyhow::Result<Address> {
        let buf: [u8; 32] = self.0 .0 .0;
        let pubkey = XOnlyPublicKey::from_slice(&buf)?;

        Ok(Address::p2tr_tweaked(
            pubkey.dangerous_assume_tweaked(),
            network,
        ))
    }
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use bitcoin::{
        hashes::Hash,
        key::{Keypair, Secp256k1},
        opcodes::all::OP_CHECKSIG,
        secp256k1::{All, SecretKey},
        taproot::TaprootBuilder,
        Address, Network, ScriptBuf, TapNodeHash, Txid, XOnlyPublicKey,
    };

    use super::{BitcoinAddress, BorshDeserialize, BorshSerialize, OutPoint, OutputRef, XOnlyPk};

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
    fn arbitrary_impl_for_outputref_works() {
        let data = vec![1u8; 100];
        let mut unstructured = Unstructured::new(&data);
        let random_outputref = OutputRef::arbitrary(&mut unstructured);

        assert!(
            random_outputref.is_ok(),
            "could not generate arbitrary outputref"
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

    fn get_random_pubkey_from_slice(secp: &Secp256k1<All>, buf: &[u8]) -> XOnlyPublicKey {
        let sk = SecretKey::from_slice(buf).unwrap();
        let keypair = Keypair::from_secret_key(secp, &sk);
        let (pk, _) = XOnlyPublicKey::from_keypair(&keypair);

        pk
    }
}
