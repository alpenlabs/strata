//! For message routing, deduplication, enforcing operator bandwidth, processing and validation,
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use strata_config::bridge::RelayerConfig;
use strata_primitives::relay::types::{BridgeMessage, Scope};
use strata_status::StatusChannel;
use strata_storage::ops::bridge_relay::BridgeMsgOps;
use strata_tasks::TaskExecutor;
use tokio::{select, sync::mpsc, time::interval};
use tracing::*;

use crate::recent_msg_tracker::RecentMessageTracker;

/// Contains bookkeeping for deduplicating messages and persisting them to disk.
pub struct RelayerState {
    /// Relayer configuration.
    config: RelayerConfig,

    /// Database interface.
    brmsg_ops: Arc<BridgeMsgOps>,

    /// To fetch the chainstate to inspect the operator set.
    status_channel: StatusChannel,

    /// Tracker to avoid duplicating messages.
    processed_msgs: RecentMessageTracker,
}

impl RelayerState {
    /// Creates a new instance.
    pub fn new(
        config: RelayerConfig,
        brmsg_ops: Arc<BridgeMsgOps>,
        status_channel: StatusChannel,
    ) -> Self {
        Self {
            config,
            brmsg_ops,
            status_channel,
            processed_msgs: RecentMessageTracker::new(),
        }
    }

    /// Handles new incoming messages by validating, enforcing bandwidth limits, checking
    /// duplicates, updating the chain state, and storing the message in the database.
    ///
    /// # Arguments
    ///
    /// * `message` - The incoming [`BridgeMessage`] to be processed.
    async fn handle_new_message(&mut self, message: BridgeMessage) -> anyhow::Result<()> {
        let message_id = message.compute_id();

        // Check for duplicates
        let timestamp = get_now_micros();
        if !self
            .processed_msgs
            .check_should_relay(timestamp, message_id)
            .await
        {
            trace!(%message_id, "dropping message we've already seen");
            return Ok(());
        }

        // Check if the message is a "misc" message.  If it is, then we should
        // only relay it if we've been set to in the config.
        let is_misc = if let Some(Scope::Misc) = message.try_parse_scope() {
            if !self.config.relay_misc {
                return Ok(());
            }

            debug!("relaying misc message");
            true
        } else {
            false
        };

        // If it's not a misc message, then we want to actually do deeper
        // validation on it.
        if !is_misc {
            // We only perform the deeper validation if we're properly synced.
            // Otherwise it's better for network health to relay them
            // unconditionally.
            // TODO make it configurable if we relay or not without chainstate?
            if let Some(op_table) = self.status_channel.operator_table() {
                let sig_res =
                    strata_primitives::relay::util::verify_bridge_msg_sig(&message, &op_table);

                if let Err(e) = sig_res {
                    trace!(err = %e, "dropping invalid message");
                    return Ok(());
                }
            }
        }

        // Store it in database.
        self.brmsg_ops.write_msg_async(timestamp, message).await?;

        Ok(())
    }

    /// Prunes old messages that are older than the specified threshold from the database and
    /// internal state.
    ///
    /// # Arguments
    ///
    /// * `before_ts` - The cutoff Unix timestamp; messages older than this will be pruned.
    async fn prune_old_msg_before(&mut self, before_ts: u128) -> anyhow::Result<()> {
        // check UNIX time and remove very old messages
        self.brmsg_ops
            .delete_msgs_before_timestamp_async(before_ts)
            .await?;

        // remove from the processed message here
        self.processed_msgs.clear_stale_messages(before_ts).await;
        Ok(())
    }
}

pub struct RelayerHandle {
    brmsg_tx: mpsc::Sender<BridgeMessage>,
    ops: Arc<BridgeMsgOps>,
}

impl RelayerHandle {
    pub async fn submit_message_async(&self, msg: BridgeMessage) {
        if let Err(msg) = self.brmsg_tx.send(msg).await {
            let msg_id = msg.0.compute_id();
            error!(%msg_id, "failed to submit bridge msg");
        }
    }

    pub fn submit_message_blocking(&self, msg: BridgeMessage) {
        if let Err(msg) = self.brmsg_tx.blocking_send(msg) {
            let msg_id = msg.0.compute_id();
            error!(%msg_id, "failed to submit bridge msg");
        }
    }

    // TODO refactor this to not require vec
    pub async fn get_message_by_scope_async(
        &self,
        scope: Vec<u8>,
    ) -> anyhow::Result<Vec<BridgeMessage>> {
        // TODO refactor this to handle errors
        Ok(self.ops.get_msgs_by_scope_async(scope).await?)
    }

    // TODO refactor this to not require vec
    pub fn get_messages_by_scope_blocking(
        &self,
        scope: Vec<u8>,
    ) -> anyhow::Result<Vec<BridgeMessage>> {
        Ok(self.ops.get_msgs_by_scope_blocking(scope)?)
    }
}

/// Starts the bridge relayer task, returning a handle to submit new messages
/// for processing.
// TODO make this a builder
pub fn start_bridge_relayer_task(
    ops: Arc<BridgeMsgOps>,
    status_channel: StatusChannel,
    config: RelayerConfig,
    task_exec: &TaskExecutor,
) -> Arc<RelayerHandle> {
    // TODO wrap the messages in a container so we make sure not to send them to
    // the peer that sent them to us
    let (brmsg_tx, brmsg_rx) = mpsc::channel::<BridgeMessage>(100);

    let state = RelayerState::new(config, ops.clone(), status_channel);
    task_exec.spawn_critical_async("bridge-msg-relayer", relayer_task(state, brmsg_rx));

    let h = RelayerHandle { brmsg_tx, ops };
    Arc::new(h)
}

async fn relayer_task(
    mut state: RelayerState,
    mut message_rx: mpsc::Receiver<BridgeMessage>,
) -> anyhow::Result<()> {
    // arbitrary refresh interval to refresh the number of message particular operator can send
    let mut refresh_interval = interval(Duration::from_secs(state.config.refresh_interval));
    loop {
        select! {
            Some(new_message) = message_rx.recv() => {
                let bmsg_id = new_message.compute_id();
                trace!(%bmsg_id, "new bridge msg");

                if let Err(e) = state.handle_new_message(new_message).await {
                    error!(err = %e, "failed to handle new message");
                }
            }

            _ = refresh_interval.tick() => {
                // prune old messages that cross the threshold duration
                let duration = get_now_micros() - state.config.stale_duration as u128 * 1_000_000;
                if let Err(e) = state.prune_old_msg_before(duration).await {
                    warn!(err = %e, "failed to purge stale messages");
                }
            }
        }
    }
}

fn get_now_micros() -> u128 {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    duration.as_micros()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use strata_primitives::relay::types::BridgeMsgId;
    use strata_test_utils::ArbitraryGenerator;

    use super::*;

    #[tokio::test]
    async fn test_add_and_check_duplicate_message() {
        let processed_msgs = RecentMessageTracker::new();

        let message_id: BridgeMsgId = ArbitraryGenerator::new().generate();

        // Add the message
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();

        // Initially, the message should not be marked as duplicate
        assert!(processed_msgs.check_should_relay(ts, message_id).await);

        // Now it should be marked as duplicate
        assert!(!processed_msgs.check_should_relay(ts, message_id).await);
    }

    #[tokio::test]
    async fn test_clear_old_messages() {
        let processed_msgs = RecentMessageTracker::new();

        // Create valid BridgeMsgId instances for testing
        let mut ag = ArbitraryGenerator::new();
        let cur_message_id: BridgeMsgId = ag.generate();
        let old_message_id: BridgeMsgId = ag.generate();
        assert_ne!(cur_message_id, old_message_id);

        // Add messages with different timestamps
        let cur_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let cur_ts_us = cur_ts.as_micros();
        let old_ts = cur_ts - Duration::from_secs(200);
        let old_ts_us = old_ts.as_micros();

        // Both messages should be considered processed initially
        assert!(
            processed_msgs
                .check_should_relay(cur_ts_us, cur_message_id)
                .await
        );
        assert!(
            processed_msgs
                .check_should_relay(old_ts_us, old_message_id)
                .await
        );

        // Clear old messages
        processed_msgs.clear_stale_messages(cur_ts_us - 1).await;

        // The old message should no longer be considered processed
        assert!(
            processed_msgs
                .check_should_relay(old_ts_us, old_message_id)
                .await
        );

        // The current message should still be considered processed
        assert!(
            !processed_msgs
                .check_should_relay(cur_ts_us, cur_message_id)
                .await
        );
    }
}
