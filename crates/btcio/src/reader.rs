use std::str::from_utf8;

use anyhow::anyhow;
use bitcoin::{consensus::deserialize, Block, Transaction};
use tracing::*;
use zeromq::{Socket, SocketRecv, SubSocket, ZmqMessage};

const HASH_BLOCK: &str = "hashblock";
const RAW_BLOCK: &str = "rawblock";
const RAW_TX: &str = "rawtx";

const SUBSCRIPTION_TOPICS: &[&'static str] = &[HASH_BLOCK, RAW_BLOCK, RAW_TX];

type Index = u32;

pub struct BtcReader {
    zmq_socket: SubSocket,
    // rpc_client: Client,
}

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

impl BtcReader {
    pub async fn new(addr: &str) -> anyhow::Result<Self> {
        let mut zmq_socket = SubSocket::new();

        // TODO: catchup with missed blocks before the connection

        info!(%addr, "Connecting to zmq socket");
        zmq_socket.connect(addr).await.expect("Failed to connect");
        info!(%addr, "Connected to zmq socket");

        for &topic in SUBSCRIPTION_TOPICS {
            zmq_socket.subscribe(topic).await?;
        }
        Ok(Self { zmq_socket })
    }

    pub async fn run<F>(&mut self, handler: F) -> anyhow::Result<()>
    where
        F: Fn(L1Data) -> anyhow::Result<()>,
    {
        loop {
            match self.zmq_socket.recv().await {
                Ok(msg) => match self.handle_message(msg, &handler) {
                    Ok(_) => {}
                    Err(_) => {}
                },
                Err(e) => {
                    error!(err = %e, "Failed to receive ZMQ message, attempting to reconnect..");
                    self.reconnect().await?;
                }
            }
        }
    }

    fn handle_message<F>(&self, msg: ZmqMessage, handler: &F) -> anyhow::Result<()>
    where
        F: Fn(L1Data) -> anyhow::Result<()>,
    {
        match msg.into_vec().as_slice() {
            [topic_bytes, data_bytes, _rest @ ..] => {
                let data = self.parse_l1_data(from_utf8(topic_bytes).unwrap(), data_bytes)?;
                handler(data)?;
            }
            _ => {
                warn!("Invalid message received from zmq");
            }
        };
        Ok(())
    }

    async fn reconnect(&self) -> anyhow::Result<()> {
        todo!()
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

pub enum L1Data<'a> {
    BlockData(BlockData<'a>),
    // TODO: add other as needed
}
