use bitcoin::ScriptBuf;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    buf::Buf32,
    l1::BitcoinAddress,
    params::{DepositTxParams, RollupParams},
};

use crate::utils::{generate_taproot_address, get_operator_wallet_pks};

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct TxFilterConfig {
    /// For checkpoint update inscriptions.
    pub rollup_name: String,

    /// For addresses that we expect spends to.
    // TODO: ensure sorted vec, possibly by having a separate SortedVec type
    pub expected_script_pubkeys: Vec<ExpectedScriptPubkey>,

    /// For blobs we expect to be written.
    pub expected_blobs: Vec<Buf32>,

    /// For deposits that might be spent from.
    pub expected_outpoints: Vec<Outpoint>,

    /// Deposit config that defines the structure we expect in the utxo
    pub deposit_config: DepositTxParams,
}

impl TxFilterConfig {
    // TODO: this will need chainstate too in the future and possibly be outside this impl
    pub fn from_rollup_params(rollup_params: &RollupParams) -> anyhow::Result<Self> {
        let operator_wallet_pks = get_operator_wallet_pks(rollup_params);
        let address = generate_taproot_address(&operator_wallet_pks, rollup_params.network)?;

        let rollup_name = rollup_params.rollup_name.clone();
        let expected_blobs = Vec::new(); // TODO: this should come from chainstate
        let expected_addrs = vec![ExpectedScriptPubkey::new_deposit_addr(address.clone())];
        let expected_outpoints = Vec::new();

        let deposit_config = DepositTxParams {
            magic_bytes: rollup_name.clone().into_bytes().to_vec(),
            address_length: rollup_params.address_length,
            deposit_amount: rollup_params.deposit_amount,
            address,
        };
        Ok(Self {
            rollup_name,
            expected_blobs,
            expected_script_pubkeys: expected_addrs,
            expected_outpoints,
            deposit_config,
        })
    }
}

/// Outpoint of a bitcoin tx
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct Outpoint {
    pub txid: Buf32,
    pub vout: u32,
}

#[derive(Clone, Debug)]
pub struct ExpectedScriptPubkey {
    // The script pubkey which we expect the input is spent to
    pub script: ScriptBuf,
    // The type of data to parse
    pub parse_type: ParseType,
}

impl BorshSerialize for ExpectedScriptPubkey {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        // Serialize the script as bytes
        let script_bytes = self.script.as_bytes();
        BorshSerialize::serialize(&script_bytes, writer)?;

        // Serialize the parse_type
        self.parse_type.serialize(writer)?;

        Ok(())
    }
}

impl BorshDeserialize for ExpectedScriptPubkey {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> Result<Self, std::io::Error> {
        // Deserialize the script as bytes and convert it back to ScriptBuf
        let script_bytes: Vec<u8> = BorshDeserialize::deserialize_reader(reader)?;
        let script = ScriptBuf::from(script_bytes);

        // Deserialize the parse_type
        let parse_type = ParseType::deserialize_reader(reader)?;

        Ok(ExpectedScriptPubkey { script, parse_type })
    }
}

impl ExpectedScriptPubkey {
    pub fn new_deposit_addr(addr: BitcoinAddress) -> Self {
        Self {
            script: addr.address().script_pubkey(),
            parse_type: ParseType::Deposit,
        }
    }

    pub fn new_deposit_req_addr(addr: BitcoinAddress) -> Self {
        Self {
            script: addr.address().script_pubkey(),
            parse_type: ParseType::DepositRequest,
        }
    }
}

impl PartialEq for ExpectedScriptPubkey {
    fn eq(&self, other: &Self) -> bool {
        self.script == other.script
    }
}
impl Eq for ExpectedScriptPubkey {}

impl PartialOrd for ExpectedScriptPubkey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ExpectedScriptPubkey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.script.cmp(&other.script)
    }
}

/// `ParseType` indicates what kind of data is to be parsed from the utxo
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub enum ParseType {
    Deposit,
    DepositRequest,
}
