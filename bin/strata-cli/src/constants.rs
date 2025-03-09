use std::time::Duration;

use alloy::consensus::constants::ETH_TO_WEI;
use bdk_wallet::bitcoin::{Amount, Network};

/// Number of blocks after bridge in transaction confirmation that the recovery path can be spent.
pub const RECOVER_DELAY: u32 = 1008;

/// Number of blocks after which we'll actually attempt recovery. This is mostly to account for any
/// reorgs that may happen at the recovery height.
pub const RECOVER_AT_DELAY: u32 = RECOVER_DELAY + 10;

pub const RECOVERY_DESC_CLEANUP_DELAY: u32 = 100;

/// 10 BTC + 0.01 to cover fees in the following transaction where the operator spends it into the
/// federation.
pub const BRIDGE_IN_AMOUNT: Amount = Amount::from_sat(1_001_000_000);

/// Bridge outs are enforced to be exactly 10 BTC
pub const BRIDGE_OUT_AMOUNT: Amount = Amount::from_int_btc(10);

pub const BTC_TO_WEI: u128 = ETH_TO_WEI;
pub const SATS_TO_WEI: u128 = BTC_TO_WEI / 100_000_000;

/// Length of salt used for password hashing
pub const PW_SALT_LEN: usize = 16;
/// Length of nonce in bytes
pub const AES_NONCE_LEN: usize = 12;
/// Length of seed in bytes
pub const SEED_LEN: usize = 16;
/// AES-256-GCM-SIV tag len
pub const AES_TAG_LEN: usize = 16;

pub const DEFAULT_NETWORK: Network = Network::Signet;
pub const BRIDGE_STRATA_ADDRESS: &str = "0x5400000000000000000000000000000000000001";
pub const SIGNET_BLOCK_TIME: Duration = Duration::from_secs(30);

pub const BRIDGE_MUSIG2_PUBKEY: &str =
    "14ced579c6a92533fa68ccc16da93b41073993cfc6cc982320645d8e9a63ee65";

/// BIP44 purpose index for HD wallets.
///
/// These should be _hardened_ [`ChildNumber`].
pub const BIP44_HD_WALLET_IDX: u32 = 44;

/// BIP44 coin type index to indicate Testnet.
///
/// These should be _hardened_ [`ChildNumber`].
pub const BIP44_ETHEREUM_MAINNET_IDX: u32 = 60;

/// BIP44 account index for user wallets.
///
/// These should be _hardened_ [`ChildNumber`].
pub const BIP44_USER_ACCOUNT_IDX: u32 = 0;

/// BIP44 change index for receiving (external) addresses.
///
/// These should be a normal [`ChildNumber`].
pub const BIP44_RECEIVING_ADDRESS_IDX: u32 = 0;

/// BIP44 address index.
///
/// These should be a normal [`ChildNumber`].
pub const BIP44_ADDRESS_IDX: u32 = 0;
