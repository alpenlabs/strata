//! Input-output with Bitcoin, implementing L1 chain trait.

use std::str::from_utf8;

use anyhow::anyhow;
use bitcoin::{consensus::deserialize, Block, BlockHash, Transaction};
use tracing::*;
use zeromq::{Socket, SocketRecv, SubSocket};

pub struct BtcIO {
    zmq_socket: SubSocket,
    // rpc_client: Client,
}

const HASH_BLOCK: &str = "hashblock";
const RAW_BLOCK: &str = "rawblock";
const RAW_TX: &str = "rawtx";

const SUBSCRIPTION_TOPICS: &[&'static str] = &[HASH_BLOCK, RAW_BLOCK, RAW_TX];

type Index = u32;
/// Store the bitcoin block and references to the relevant transactions within the block
pub struct BlockData<'a> {
    block: Block,
    relevant_txns: Vec<(Index, &'a Transaction)>,
}

impl<'a> BlockData<'a> {
    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn relevant_txns(&self) -> &Vec<(Index, &'a Transaction)> {
        &self.relevant_txns
    }
}

pub enum L1Data<'a> {
    BlockData(BlockData<'a>),
    // TODO: add other as needed
}

impl BtcIO {
    pub async fn new(addr: &str) -> anyhow::Result<Self> {
        let mut zmq_socket = SubSocket::new();
        info!("Connecting to zmq socket");
        zmq_socket.connect(addr).await.expect("Failed to connect");
        info!("Connected to zmq socket");
        for &topic in SUBSCRIPTION_TOPICS {
            zmq_socket.subscribe(topic).await?;
        }
        Ok(Self { zmq_socket })
    }

    pub async fn run<F>(&mut self, handler: F) -> anyhow::Result<()>
    where
        F: Fn(L1Data) -> anyhow::Result<()>,
    {
        info!("Running zmq");
        while let Some(msg) = self.zmq_socket.recv().await.ok() {
            // info!("msg {:?}", msg);
            match msg.into_vec().as_slice() {
                [topic_bytes, data_bytes, _rest @ ..] => {
                    let data = self.parse_l1_data(from_utf8(topic_bytes).unwrap(), data_bytes)?;
                    handler(data)?;
                }
                _ => {
                    warn!("Invalid message received from zmq");
                }
            };
        }
        Err(anyhow!("Failed to receive message from zmq socket"))
    }

    fn parse_l1_data(&self, topic: &str, msg: &bytes::Bytes) -> anyhow::Result<L1Data> {
        // TODO: clean this
        match topic {
            RAW_BLOCK => {
                let block: Block = deserialize(&msg.to_vec())?;
                // TODO: extract relevant txns
                return Ok(L1Data::BlockData(BlockData {
                    block,
                    relevant_txns: vec![],
                }));
            }
            _ => {
                warn!("Something else obtained");
            }
        }
        Err(anyhow!("Inalid data"))
    }
}
