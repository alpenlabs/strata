use std::str::from_utf8;

use anyhow::anyhow;
use bitcoin::{consensus::deserialize, Block};
use tracing::*;
use zeromq::{Socket, SocketRecv, SubSocket, ZmqMessage};

const HASH_BLOCK: &str = "hashblock";
const RAW_BLOCK: &str = "rawblock";
const RAW_TX: &str = "rawtx";

const SUBSCRIPTION_TOPICS: &[&'static str] = &[HASH_BLOCK, RAW_BLOCK, RAW_TX];

pub struct BtcReader<F>
where
    F: Fn(BlockData) -> anyhow::Result<()>,
{
    /// The zmq socket that listens to bitcoin notifications
    zmq_socket: SubSocket,

    /// This keep track of sequence of block notifications from zmq
    block_seq: Option<u32>,

    /// The last block number that the reader has successfully processed
    last_block_number: u64,

    /// Handler that handles the data received from L1
    handler: F,
}

/// Store the bitcoin block and references to the relevant transactions within the block
pub struct BlockData {
    block_num: u64,
    block: Block,
    relevant_txn_indices: Vec<u32>,
}

impl BlockData {
    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn relevant_txn_indices(&self) -> &Vec<u32> {
        &self.relevant_txn_indices
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}

impl<F> BtcReader<F>
where
    F: Fn(BlockData) -> anyhow::Result<()>,
{
    pub async fn new(addr: &str, synced_block_num: u64, handler: F) -> anyhow::Result<Self> {
        let mut zmq_socket = SubSocket::new();

        info!(%addr, "Connecting to zmq socket");
        zmq_socket.connect(addr).await.expect("Failed to connect");
        info!(%addr, "Connected to zmq socket");

        let mut reader = Self {
            zmq_socket,
            block_seq: None,
            last_block_number: synced_block_num,
            handler,
        };

        // Catchup with missed blocks before subscription
        reader.sync_since(synced_block_num + 1).await?;

        for &topic in SUBSCRIPTION_TOPICS {
            reader.zmq_socket.subscribe(topic).await?;
        }
        Ok(reader)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            match self.zmq_socket.recv().await {
                Ok(msg) => self.handle_message(msg).await?,
                Err(e) => {
                    error!(err = %e, "Failed to receive ZMQ message, attempting to reconnect..");
                    self.reconnect().await?;
                }
            }
        }
    }

    async fn handle_message(&mut self, msg: ZmqMessage) -> anyhow::Result<()> {
        match msg.into_vec().as_slice() {
            [topic_bytes, data_bytes, seq_bytes] => {
                let blockdata =
                    self.parse_block_data(from_utf8(topic_bytes).unwrap(), data_bytes)?;

                let seq: u32 = parse_sequence(seq_bytes)?;

                if !self.is_sequence_in_order(seq) {
                    self.recover_out_of_order(seq).await?;
                }

                let block_num = blockdata.block_num;

                (self.handler)(blockdata)?;

                self.block_seq = Some(seq);
                self.last_block_number = block_num;
            }
            _ => {
                warn!("Invalid message received from zmq");
            }
        };
        Ok(())
    }

    async fn recover_out_of_order(&mut self, seq: u32) -> anyhow::Result<()> {
        match self.block_seq {
            Some(bseq) => {
                let seq_diff = seq - bseq - 1;
                self.sync_between(
                    self.last_block_number,
                    self.last_block_number + (seq_diff as u64),
                )
                .await?;
            }
            None => {}
        };
        Ok(())
    }

    fn is_sequence_in_order(&self, seq: u32) -> bool {
        if let Some(s) = self.block_seq {
            return s + 1 == seq;
        }
        return true;
    }

    async fn reconnect(&self) -> anyhow::Result<()> {
        todo!()
    }

    fn parse_block_data(&self, topic: &str, msg: &bytes::Bytes) -> anyhow::Result<BlockData> {
        match topic {
            RAW_BLOCK => {
                // FIXME: We'll probably need block number, which zmq does not provide.
                // Seems like we need a rpc call for every block received.
                let block_num = 0; // TODO:
                let block: Block = deserialize(&msg.to_vec())?;
                // TODO: extract relevant txns
                return Ok(BlockData {
                    block_num,
                    block,
                    relevant_txn_indices: vec![],
                });
            }
            _ => {
                warn!("Something else obtained");
            }
        }
        Err(anyhow!("Inalid data"))
    }

    /// Fetch the blocks since `block_num` until latest
    async fn sync_since(&mut self, block_num: u64) -> anyhow::Result<()> {
        // uses bitcoin rpc

        // TODO: 1. get latest block hash
        // TODO: 2. get block info and hence latest block num
        // TODO: 3. self.sync_between(block_num, latest_block_num)
        Ok(())
    }

    async fn sync_between(
        &mut self,
        start_block_num: u64,
        end_block_num: u64,
    ) -> anyhow::Result<()> {
        // uses bitcoin rpc
        // TODO: this probably needs to be atomic

        Ok(())
    }
}

fn parse_sequence(seq_bytes: &bytes::Bytes) -> anyhow::Result<u32> {
    if seq_bytes.len() != 4 {
        return Err(anyhow!("Invalid sequence bytes"));
    }
    let mut arr: [u8; 4] = [0; 4];
    arr.copy_from_slice(seq_bytes);
    Ok(u32::from_le_bytes(arr))
}
