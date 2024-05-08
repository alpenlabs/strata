use reth_primitives::alloy_primitives::FixedBytes;

// 20-byte buf
pub type Buf20 = FixedBytes<20>;

// 32-byte buf, useful for hashes and schnorr pubkeys
pub type Buf32 = FixedBytes<32>;

// 64-byte buf, useful for schnorr signatures
pub type Buf64 = FixedBytes<64>;
