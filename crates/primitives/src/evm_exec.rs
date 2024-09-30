use borsh::{BorshDeserialize, BorshSerialize};
use hex::FromHex;
use reth_primitives::B256;

use crate::buf::Buf32;

/// Structure for `ExecUpdate.input.extra_payload` for EVM EL
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct EVMExtraPayload {
    block_hash: [u8; 32],
}

impl EVMExtraPayload {
    pub fn new(block_hash: [u8; 32]) -> Self {
        Self { block_hash }
    }

    pub fn block_hash(&self) -> Buf32 {
        self.block_hash.into()
    }
}

/// Generate extra_payload for evm el
pub fn create_evm_extra_payload(block_hash: Buf32) -> Vec<u8> {
    let extra_payload = EVMExtraPayload {
        block_hash: *block_hash.as_ref(),
    };
    borsh::to_vec(&extra_payload).expect("extra_payload vec")
}

#[test]
fn test_onece() {
    let data: B256 =
        B256::from_hex("0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba")
            .unwrap();
    let buf = Buf32::from(data.0);

    let out = create_evm_extra_payload(buf);

    println!("{:?}", out);
}
