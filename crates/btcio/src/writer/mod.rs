mod builder;

use std::{str::FromStr, sync::Arc};

use bitcoin::Address;
use tokio::sync::{broadcast::Receiver, mpsc};

use crate::rpc::{types::RawUTXO, BitcoinClient};

use self::builder::{create_inscription_transactions, UtxoParseError, UTXO};

enum WriterMsg {}

// TODO: this should be somewhere common to duty executor
#[derive(Clone)]
pub struct L1WriteIntent {
    /// The range of L2 blocks that the intent spans
    pub block_range: (u64, u64),

    /// Proof of the batch execution
    pub proof_data: Vec<u8>, // TODO: maybe typed serializable data

    /// Sequencer's proof signature
    pub proof_signature: Vec<u8>,

    /// Actual batch data to be posted. Possible state-diff
    pub batch_data: Vec<u8>, // TODO: maybe typed serializable data

    /// Sequencer's Batch signature
    pub batch_signature: Vec<u8>,
}

// TODO: this comes from config or inside L1WriteIntent
const SEQUENCER_PUBKEY: &[u8] = &[];
// This probably should be in config, or we can just pay dust
const AMOUNT_TO_REVEAL_TXN: u64 = 1000;
const ROLLUP_NAME: &str = "alpen";

pub async fn writer_control_task(
    mut duty_receiver: Receiver<L1WriteIntent>,
    rpc_client: Arc<BitcoinClient>,
) -> anyhow::Result<()> {
    let (sender, receiver) = mpsc::channel::<WriterMsg>(100);
    let change_address = Address::from_str("000")?.require_network(rpc_client.network())?;

    tokio::spawn(watch_and_retry_task());

    loop {
        let write_intent = duty_receiver.recv().await?;
        let utxos = rpc_client.get_utxos().await?;
        let utxos = utxos
            .into_iter()
            .map(|x| <RawUTXO as TryInto<UTXO>>::try_into(x))
            .into_iter()
            .collect::<Result<Vec<UTXO>, UtxoParseError>>()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?;

        let fee_rate = rpc_client.estimate_smart_fee().await?;
        let _ = create_inscription_transactions(
            ROLLUP_NAME,
            write_intent,
            SEQUENCER_PUBKEY.to_vec(),
            utxos,
            change_address.clone(),
            AMOUNT_TO_REVEAL_TXN,
            fee_rate,
            rpc_client.network(),
        )?;

        // send to bitcoin
    }
}

/// Watches for inscription transactions status in bitcion and retries until they are confirmed
pub async fn watch_and_retry_task() {}
