use std::collections::HashMap;

use bitcoin::{
    block::Header,
    consensus::{deserialize, serialize},
    hashes::Hash,
    Block, Transaction,
};
use strata_primitives::{buf::Buf32, l1::L1BlockRecord};
use strata_state::l1::{
    get_difficulty_adjustment_height, BtcParams, HeaderVerificationState, L1BlockId, TimestampStore,
};
use strata_tx_parser::filter::TxFilterConfig;

use crate::{l2::gen_params, ArbitraryGenerator};

pub fn get_test_bitcoin_txs() -> Vec<Transaction> {
    let t1 = "0200000000010176f29f18c5fc677ad6dd6c9309f6b9112f83cb95889af21da4be7fbfe22d1d220000000000fdffffff0300e1f505000000002200203946555814a18ccc94ef4991fb6af45278425e6a0a2cfc2bf4cf9c47515c56ff0000000000000000176a1500e0e78c8201d91f362c2ad3bb6f8e6f31349454663b1010240100000022512012d77c9ae5fdca5a3ab0b17a29b683fd2690f5ad56f6057a000ec42081ac89dc0247304402205de15fbfb413505a3563608dad6a73eb271b4006a4156eeb62d1eacca5efa10b02201eb71b975304f3cbdc664c6dd1c07b93ac826603309b3258cb92cfd201bb8792012102f55f96fd587a706a7b5e7312c4e9d755a65b3dad9945d65598bca34c9e961db400000000";
    let t2 = "02000000000101f4f2e8830d2948b5e980e739e61b23f048d03d4af81588bf5da4618406c495aa0000000000fdffffff02969e0700000000002200203946555814a18ccc94ef4991fb6af45278425e6a0a2cfc2bf4cf9c47515c56ff60f59000000000001600148d0499ec043b1921a608d24690b061196e57c927040047304402203875f7b610f8783d5f5c163118eeec1a23473dd33b53c8ea584c7d28a82b209b022034b6814344b79826a348e23cc19ff06ed2df23850b889557552e376bf9e32c560147304402200f647dad3c137ff98d7da7a302345c82a57116a3d0e6a3719293bbb421cb0abe02201c04a1e808f5bab3595f77985af91aeaf61e9e042c9ac97d696e0f4b020cb54b0169522102dba8352965522ff44538dde37d793b3b4ece54e07759ade5f648aa396165d2962103c0683712773b725e7fe4809cbc90c9e0b890c45e5e24a852a4c472d1b6e9fd482103bf56f172d0631a7f8ae3ef648ad43a816ad01de4137ba89ebc33a2da8c48531553ae00000000";
    let t3 = "02000000000101f4f2e8830d2948b5e980e739e61b23f048d03d4af81588bf5da4618406c495aa0200000000ffffffff0380969800000000002200203946555814a18ccc94ef4991fb6af45278425e6a0a2cfc2bf4cf9c47515c56ff0000000000000000176a15006e1a916a60b93a545f2370f2a36d2f807fb3d675588b693a000000001600149fafc79c72d1c4d917a360f32bdc68755402ef670247304402203c813ad8918366ce872642368b57b78e78e03b1a1eafe16ec8f3c9268b4fc050022018affe880963f18bfc0338f1e54c970185aa90f8c36a52ac935fe76cb885d726012102fa9b81d082a98a46d0857d62e6c9afe9e1bf40f9f0cbf361b96241c9d6fb064b00000000";
    let t4 = "02000000000101d8acf0a647b7d5d1d0ee83360158d5bf01146d3762c442defd7985476b02aa6b0100000000fdffffff030065cd1d000000002200203946555814a18ccc94ef4991fb6af45278425e6a0a2cfc2bf4cf9c47515c56ff0000000000000000176a1500e0e78c8201d91f362c2ad3bb6f8e6f3134945466aec19dd00000000022512040718748dbca6dea8ac6b6f0b177014f0826478f1613c2b489e738db7ecdf3610247304402207cfc5cd87ec83687c9ac2bd921e96b8a58710f15d77bc7624da4fb29fe589dab0220437b74ed8e8f9d3084269edfb8641bf27246b0e5476667918beba73025c7a2c501210249a34cfbb6163b1b6ca2fff63fd1f8a802fb1999fa7930b2febe5a711f713dd900000000";
    let t5 = "0200000000010176f29f18c5fc677ad6dd6c9309f6b9112f83cb95889af21da4be7fbfe22d1d220000000000fdffffff0300e1f505000000002200203946555814a18ccc94ef4991fb6af45278425e6a0a2cfc2bf4cf9c47515c56ff0000000000000000176a1500e0e78c8201d91f362c2ad3bb6f8e6f31349454663b1010240100000022512012d77c9ae5fdca5a3ab0b17a29b683fd2690f5ad56f6057a000ec42081ac89dc0247304402205de15fbfb413505a3563608dad6a73eb271b4006a4156eeb62d1eacca5efa10b02201eb71b975304f3cbdc664c6dd1c07b93ac826603309b3258cb92cfd201bb8792012102f55f96fd587a706a7b5e7312c4e9d755a65b3dad9945d65598bca34c9e961db400000000";
    [t1, t2, t3, t4, t5]
        .iter()
        .map(|x| deserialize(&hex::decode(x).unwrap()).unwrap())
        .collect()
}

pub fn gen_l1_chain(len: usize) -> Vec<L1BlockRecord> {
    // FIXME this is bad, the blocks generated are nonsensical
    let mut blocks = vec![];
    for _ in 0..len {
        let block: L1BlockRecord = ArbitraryGenerator::new().generate();
        blocks.push(block);
    }
    blocks
}

pub fn get_btc_mainnet_block() -> Block {
    let raw_block = include_bytes!(
        "../data/mainnet_block_000000000000000000000c835b2adcaedc20fdf6ee440009c249452c726dafae.raw"
    );
    let block: Block = deserialize(&raw_block[..]).unwrap();
    block
}

pub struct BtcChainSegment {
    pub headers: Vec<Header>,
    pub start: u32,
    pub end: u32,
    pub custom_blocks: HashMap<u32, Block>,
}

impl BtcChainSegment {
    /// Retrieves the block header at the specified height.
    pub fn get_header(&self, height: u32) -> Header {
        assert!(
            (self.start..self.end).contains(&height),
            "height must be in the range [{}..{})",
            self.start,
            self.end
        );
        let idx = height - self.start;
        self.headers[idx as usize]
    }

    pub fn get_block(&self, height: u32) -> &Block {
        self.custom_blocks.get(&height).unwrap()
    }

    /// Retrieves the timestamps of a specified number of blocks from a given height in a
    /// descending order.
    pub fn get_last_timestamps(&self, from: u32, count: u32) -> Vec<u32> {
        let mut timestamps = Vec::with_capacity(count as usize);
        for i in (0..count).rev() {
            let h = self.get_header(from - i);
            timestamps.push(h.time)
        }
        timestamps
    }

    pub fn get_verification_state(
        &self,
        block_height: u32,
        params: &BtcParams,
    ) -> HeaderVerificationState {
        // Get the difficulty adjustment block just before `block_height`
        let h1 = get_difficulty_adjustment_height(0, block_height, params);

        // Consider the block before `block_height` to be the last verified block
        let vh = block_height - 1; // verified_height

        // Fetch the previous timestamps of block from `vh`
        // This fetches timestamps of `vh`, `vh-1`, `vh-2`, ...
        let initial_timestamps: [u32; 11] = self.get_last_timestamps(vh, 11).try_into().unwrap();
        let last_11_blocks_timestamps = TimestampStore::new(initial_timestamps);

        let last_verified_block_hash: L1BlockId = Buf32::from(
            self.get_header(vh)
                .block_hash()
                .as_raw_hash()
                .to_byte_array(),
        )
        .into();

        HeaderVerificationState {
            last_verified_block_num: vh,
            last_verified_block_hash,
            next_block_target: self
                .get_header(vh)
                .target()
                .to_compact_lossy()
                .to_consensus(),
            interval_start_timestamp: self.get_header(h1).time,
            total_accumulated_pow: 0u128,
            last_11_blocks_timestamps,
        }
    }

    pub fn get_block_manifest(&self, height: u32) -> L1BlockRecord {
        let header = self.get_header(height);
        L1BlockRecord::new(
            header.block_hash().into(),
            serialize(&header),
            Buf32::from(header.merkle_root.as_raw_hash().to_byte_array()),
        )
    }

    pub fn get_block_manifests(&self, from_height: u32, len: usize) -> Vec<L1BlockRecord> {
        let mut blocks = Vec::with_capacity(len);
        for i in 0..len {
            let block = self.get_block_manifest(from_height + i as u32);
            blocks.push(block);
        }
        blocks
    }
}

pub fn get_btc_chain() -> BtcChainSegment {
    let buffer = include_bytes!("../data/mainnet_blocks_40000-50000.raw");

    let chunk_size = Header::SIZE;
    let capacity = buffer.len() / chunk_size;
    let mut headers = Vec::with_capacity(capacity);

    for chunk in buffer.chunks(chunk_size) {
        let raw_header = chunk.to_vec();
        let header: Header = deserialize(&raw_header).unwrap();
        headers.push(header);
    }

    // This custom blocks are chose because this is where the first difficulty happened
    let custom_blocks: HashMap<u32, Block> = vec![
        (40321, "0100000045720d24eae33ade0d10397a2e02989edef834701b965a9b161e864500000000993239a44a83d5c427fd3d7902789ea1a4d66a37d5848c7477a7cf47c2b071cd7690784b5746651c3af7ca030101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c02db00ffffffff0100f2052a01000000434104c9f513361104db6a84fb6d5b364ba57a27cd19bd051239bf750d8999c6b437220df8fea6b932a248df3cad1fdebb501791e02b7b893a44718d696542ba92a0acac00000000".to_owned()),
        (40322, "01000000fd1133cd53d00919b0bd77dd6ca512c4d552a0777cc716c00d64c60d0000000014cf92c7edbe8a75d1e328b4fec0d6143764ecbd0f5600aba9d22116bf165058e590784b5746651c1623dbe00101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c020509ffffffff0100f2052a010000004341043eb751f57bd4839a8f2922d5bf1ed15ade9b161774658fb39801f0b9da9c881f226fbe4ee0c240915f17ce5255dd499075ab49b199a7b1f898fb20cc735bc45bac00000000".to_owned()),
        (40323, "01000000c579e586b48485b6e263b54949d07dce8660316163d915a35e44eb570000000011d2b66f9794f17393bf90237f402918b61748f41f9b5a2523c482a81a44db1f4f91784b5746651c284557020101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c024502ffffffff0100f2052a01000000434104597b934f2081e7f0d7fae03ec668a9c69a090f05d4ee7c65b804390d94266ffb90442a1889aaf78b460692a43857638520baa8319cf349b0d5f086dc4d36da8eac00000000".to_owned()),
        (40324, "010000001f35c6ea4a54eb0ea718a9e2e9badc3383d6598ff9b6f8acfd80e52500000000a7a6fbce300cbb5c0920164d34c36d2a8bb94586e9889749962b1be9a02bbf3b9194784b5746651c0558e1140101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff08045746651c029001ffffffff0100f2052a01000000434104e5d390c21b7d221e6ba15c518444c1aae43d6fb6f721c4a5f71e590288637ca2961be07ee845a795da3fd1204f52d4faa819c167062782590f08cf717475e488ac00000000".to_owned()),
        ]
        .into_iter()
        .map(|(h, raw_block)| {
            let block_bytes = hex::decode(&raw_block).unwrap();
            let block: Block = bitcoin::consensus::deserialize(&block_bytes).unwrap();
            (h, block)
        })
        .collect();

    BtcChainSegment {
        headers,
        start: 40_000,
        end: 50_000,
        custom_blocks,
    }
}

pub fn get_test_tx_filter_config() -> TxFilterConfig {
    let config = gen_params();
    TxFilterConfig::derive_from(config.rollup()).expect("can't derive filter config")
}
