//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{
    consensus::{deserialize, serialize},
    Block,
};
use borsh::{BorshDeserialize, BorshSerialize};
use strata_l1tx::filter::{indexer::index_block, TxFilterConfig};
use strata_state::{
    l1::L1TxProof,
    tx::{DaCommitment, DepositInfo, ProtocolOperation},
};
use zkaleido::ZkVmEnv;

use crate::{block::check_integrity, tx_indexer::ProverTxVisitorImpl};

/// Defines the result of scanning an L1 block.
/// Includes protocol-relevant data posted on L1 block.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BlockScanResult {
    /// Raw header of the block that we procesed
    pub raw_header: [u8; 80],
    /// Deposits that we found in the block
    pub deposits: Vec<DepositInfo>,
    /// DA Commitments that we found in the block
    pub da_commitments: Vec<DaCommitment>,
    /// Transaction Filters we used to scan the block
    // NOTE: We have to commit to the filters to later verify that we have indeed used the correct
    // filters
    pub tx_filters: TxFilterConfig,
}

/// Represents the input data required for generating an L1Scan proof.
#[derive(Debug)]
pub struct BlockScanProofInput {
    /// Full block that we use scan.
    ///
    /// Inclusion proof will be created if there are witness transactions
    pub block: Block,
    /// Tx filters we use to scan this block
    pub tx_filters: TxFilterConfig,
}

pub fn process_blockscan_proof(zkvm: &impl ZkVmEnv) {
    // 1a. Read the full serialized block and deserialize it
    let serialized_block = zkvm.read_buf();
    let block: Block = deserialize(&serialized_block).expect("invalid block serialization");

    // 1b. Read inclusion proof and tx_filters
    let inclusion_proof: Option<L1TxProof> = zkvm.read_borsh();
    let tx_filters: TxFilterConfig = zkvm.read_borsh();

    // 2. Check that the content of the block is valid
    assert!(check_integrity(&block, &inclusion_proof), "invalid block");

    // 3. Index the block for protocol ops
    let protocol_ops = index_block(&block, ProverTxVisitorImpl::new, &tx_filters);

    // 4. Collect deposits and DA commitments
    let mut deposits = Vec::new();
    let mut da_commitments = Vec::new();
    for tx_entry in protocol_ops.into_iter() {
        for op in tx_entry.into_contents() {
            match op {
                ProtocolOperation::Deposit(deposit) => deposits.push(deposit),
                ProtocolOperation::DaCommitment(commitment) => da_commitments.push(commitment),
                _ => {} // ignore other variants
            }
        }
    }

    // 5. Commit to the output
    let raw_header = serialize(&block.header)
        .try_into()
        .expect("bitcoin block header is 80 bytes");
    let output = BlockScanResult {
        raw_header,
        deposits,
        da_commitments,
        tx_filters,
    };
    zkvm.commit_borsh(&output);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use strata_test_utils::{bitcoin_mainnet_segment::BtcChainSegment, l2::gen_params};
    use zkaleido::ZkVmProver;
    use zkaleido_native_adapter::{NativeHost, NativeMachine};

    use super::*;
    use crate::prover::BtcBlockspaceProver;

    fn get_native_host() -> NativeHost {
        NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_blockscan_proof(zkvm);
                Ok(())
            })),
        }
    }

    #[test]
    fn test_process_blockspace_proof_before_segwit() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let tx_filters = TxFilterConfig::derive_from(rollup_params).unwrap();

        let btc_block = BtcChainSegment::load().get_block_at(40321).unwrap();
        let input = BlockScanProofInput {
            block: btc_block,
            tx_filters,
        };
        BtcBlockspaceProver::prove(&input, &get_native_host()).unwrap();
    }

    #[test]
    fn test_process_blockspace_proof_after_segwit() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let tx_filters = TxFilterConfig::derive_from(rollup_params).unwrap();

        let btc_block = BtcChainSegment::load_full_block();
        let input = BlockScanProofInput {
            block: btc_block,
            tx_filters,
        };
        BtcBlockspaceProver::prove(&input, &get_native_host()).unwrap();
    }
}
