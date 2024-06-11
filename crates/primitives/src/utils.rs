use bitcoin::{
    consensus::serialize,
    hashes::{sha256d, Hash},
    Block, Wtxid,
};

use crate::{
    l1::{L1Tx, L1TxProof},
    prelude::Buf32,
};

fn get_cohashes_from_wtxids(txids: &[Wtxid], index: u32) -> (Vec<Buf32>, Buf32) {
    assert!((index as usize) < txids.len());

    let mut curr_level: Vec<_> = txids
        .iter()
        .cloned()
        .map(|x| x.to_raw_hash().to_byte_array())
        .collect();
    let mut curr_index = index;
    let mut proof = Vec::new();

    while curr_level.len() > 1 {
        let len = curr_level.len();
        if len % 2 != 0 {
            curr_level.push(curr_level[len - 1].clone());
        }

        let proof_item_index = if curr_index % 2 == 0 {
            curr_index + 1
        } else {
            curr_index - 1
        };

        let item = curr_level[proof_item_index as usize];
        proof.push(Buf32(item.into()));

        // construct pairwise hash
        curr_level = curr_level
            .chunks(2)
            .map(|pair| {
                let [a, b] = pair else {
                    panic!("should be a pair");
                };
                let mut arr = [0u8; 64];
                arr[..32].copy_from_slice(a);
                arr[32..].copy_from_slice(b);
                sha256d::Hash::hash(&arr).as_byte_array().clone()
            })
            .collect::<Vec<_>>();
        curr_index = curr_index >> 1;
    }
    (proof, Buf32(curr_level[0].into()))
}

pub fn generate_l1_tx(idx: u32, block: &Block) -> L1Tx {
    assert!((idx as usize) < block.txdata.len());
    let tx = &block.txdata[idx as usize];

    let (cohashes, _wtxroot) = get_cohashes_from_wtxids(
        &block
            .txdata
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if i == 0 {
                    Wtxid::all_zeros() // Coinbase's wtxid is all zeros
                } else {
                    x.compute_wtxid()
                }
            })
            .collect::<Vec<_>>(),
        idx,
    );

    let proof = L1TxProof::new(idx, cohashes);
    let tx = serialize(tx);

    L1Tx::new(proof, tx)
}

#[cfg(test)]
mod tests {
    use bitcoin::consensus::deserialize;

    use super::*;

    fn get_test_wtxids() -> Vec<Wtxid> {
        vec![
            deserialize(&[1; 32]).unwrap(),
            deserialize(&[2; 32]).unwrap(),
            deserialize(&[3; 32]).unwrap(),
            deserialize(&[4; 32]).unwrap(),
            deserialize(&[5; 32]).unwrap(),
            deserialize(&[6; 32]).unwrap(),
            deserialize(&[7; 32]).unwrap(),
        ]
    }

    #[test]
    fn test_get_cohashes_from_wtxids_idx_2() {
        let txids: Vec<Wtxid> = get_test_wtxids();
        let index = 2;

        let (proof, root) = get_cohashes_from_wtxids(&txids, index);
        // Validate the proof length
        assert_eq!(proof.len(), 3);

        // Validate the proof contents
        assert_eq!(proof[0].0, [4; 32]);
        assert_eq!(
            proof[1].0,
            [
                57, 206, 32, 190, 222, 130, 201, 107, 137, 8, 190, 196, 161, 87, 176, 156, 84, 155,
                61, 185, 11, 155, 71, 75, 218, 154, 233, 185, 3, 3, 16, 180
            ]
        );
        assert_eq!(
            proof[2].0,
            [
                182, 31, 195, 174, 213, 89, 251, 184, 232, 133, 217, 123, 109, 127, 232, 151, 21,
                83, 204, 182, 115, 231, 30, 116, 89, 113, 163, 62, 104, 190, 1, 213
            ]
        );

        // Validate the root hash
        let expected_root = [
            92, 218, 49, 127, 159, 148, 231, 132, 215, 129, 27, 155, 152, 132, 243, 8, 47, 11, 170,
            252, 138, 147, 167, 219, 111, 149, 245, 126, 165, 46, 146, 105,
        ];
        assert_eq!(root.0, expected_root);
    }

    #[test]
    fn test_get_cohashes_from_wtxids_idx_5() {
        let txids: Vec<Wtxid> = get_test_wtxids();

        let index = 5;

        let (proof, root) = get_cohashes_from_wtxids(&txids, index);

        // Validate the proof length
        assert_eq!(proof.len(), 3);

        // Validate the proof contents
        assert_eq!(proof[0].0, [5; 32]);
        assert_eq!(
            proof[1].0,
            [
                166, 91, 23, 162, 124, 131, 204, 95, 164, 84, 106, 176, 191, 145, 187, 217, 223,
                227, 39, 192, 18, 246, 37, 176, 214, 240, 109, 242, 54, 116, 52, 57
            ]
        );
        assert_eq!(
            proof[2].0,
            [
                8, 90, 171, 174, 249, 134, 104, 112, 27, 135, 201, 161, 152, 107, 223, 17, 103, 38,
                169, 148, 152, 2, 50, 107, 105, 137, 86, 151, 212, 232, 200, 18
            ]
        );

        // Validate the root hash
        let expected_root = [
            92, 218, 49, 127, 159, 148, 231, 132, 215, 129, 27, 155, 152, 132, 243, 8, 47, 11, 170,
            252, 138, 147, 167, 219, 111, 149, 245, 126, 165, 46, 146, 105,
        ];
        assert_eq!(root.0, expected_root);
    }
}
