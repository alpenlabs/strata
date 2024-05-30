//! Input-output with Bitcoin, implementing L1 chain trait.

use std::str::from_utf8;

use anyhow::anyhow;
use bitcoin::{
    consensus::deserialize, opcodes::all::OP_RETURN, Amount, Block, BlockHash, Transaction,
};
use tracing::*;
use zeromq::{Socket, SocketRecv, SubSocket};

pub struct L1Reader {
    zmq_socket: SubSocket,
    // rpc_client: Client,
}

const HASH_BLOCK: &str = "hashblock";
const RAW_BLOCK: &str = "rawblock";
const RAW_TX: &str = "rawtx";

const SUBSCRIPTION_TOPICS: &[&'static str] = &[HASH_BLOCK, RAW_BLOCK, RAW_TX];

impl L1Reader {
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

    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("Running zmq");
        while let Some(msg) = self.zmq_socket.recv().await.ok() {
            // info!("msg {:?}", msg);
            msg.get(0).map(|topic_bytes| {
                self.handle_topic_message(from_utf8(topic_bytes).unwrap(), msg.get(1))
            });
        }
        Err(anyhow!("Failed to receive message from zmq socket"))
    }

    fn handle_topic_message(&self, topic: &str, msg: Option<&bytes::Bytes>) -> anyhow::Result<()> {
        match topic {
            HASH_BLOCK => {
                let parsed_hash: BlockHash = deserialize(&msg.unwrap().to_vec())?;
                info!("HASH BLOCK RECEIVED: {:?}", parsed_hash);
            }
            RAW_BLOCK => {
                let block: Block =
                    deserialize(&msg.unwrap().to_vec()).expect("could not parse block");
                let coinbase_wtx_utxo = &block.txdata[0]
                    .output
                    .iter()
                    .filter(|&x| {
                        x.value == Amount::ZERO
                            && x.script_pubkey.as_bytes()[0] == OP_RETURN.to_u8()
                    })
                    .next();
                info!("Coinbase RECEIVED: {:?}", coinbase_wtx_utxo);
            }
            RAW_TX => {
                let tx: Transaction =
                    deserialize(&msg.unwrap().to_vec()).expect("could not parse transaction");
                // info!("RAW TX RECEIVED: {:?}", tx);
            }
            _ => {
                warn!("Something else obtained");
            }
        }
        Ok(())
    }
}
