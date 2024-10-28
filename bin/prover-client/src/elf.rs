//! Mock ELF placeholders for the guest codes.
//!
//! These empty ELF constants are used when the prover client is built without the `prover-dev`
//! feature, allowing builds without needing actual guest code. If `prover-dev` is enabled, the
//! client will build and embed the actual ELF binaries.
#![allow(unused)]
pub const GUEST_EVM_EE_STF_ELF: &[u8] = &[];
pub const GUEST_CL_STF_ELF: &[u8] = &[];
pub const GUEST_CL_AGG_ELF: &[u8] = &[];
pub const GUEST_L1_BATCH_ELF: &[u8] = &[];
pub const GUEST_BTC_BLOCKSPACE_ELF: &[u8] = &[];
pub const GUEST_CHECKPOINT_ELF: &[u8] = &[];
