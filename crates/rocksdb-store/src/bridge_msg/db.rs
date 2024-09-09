#![allow(unused)]

use std::sync::Arc;

use alpen_express_bridge_msg::types::{BridgeMessage, Scope};
use alpen_express_db::{traits::BridgeMessageStore, DbError, DbResult};
use rockbound::{
    utils::get_last, OptimisticTransactionDB as DB, SchemaBatch, SchemaDBOperationsExt,
    TransactionRetry,
};

use super::schemas::{BridgeMsgIdSchema, ScopeMsgIdSchema};
use crate::DbOpsConfig;

pub struct BridgeMsgDb {
    db: Arc<DB>,
    ops: DbOpsConfig,
}

impl BridgeMsgDb {
    pub fn new(db: Arc<DB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }

    fn get_msg_ids_before_timestamp(&self, msg_id: u128) -> DbResult<Vec<u128>> {
        // reverse and then place a iterator here
        let mut iterator = self.db.iter::<BridgeMsgIdSchema>()?;
        iterator.seek_to_first();

        let mut ids = Vec::new();
        for res in iterator {
            let (timestamp, _) = res?.into_tuple();
            if timestamp <= msg_id {
                ids.push(timestamp);
            }
        }

        Ok(ids)
    }
}

impl BridgeMessageStore for BridgeMsgDb {
    fn write_msg(&self, id: u128, msg: BridgeMessage) -> alpen_express_db::DbResult<()> {
        let mut id = id;
        while self.db.get::<BridgeMsgIdSchema>(&id)?.is_some() {
            id += 1;
        }

        self.db.put::<BridgeMsgIdSchema>(&id, &msg);

        if let Some(scopes) = self.db.get::<ScopeMsgIdSchema>(msg.get_scope())? {
            let mut new_scopes = Vec::new();
            new_scopes.extend(&scopes);
            new_scopes.push(id);
            self.db
                .put::<ScopeMsgIdSchema>(msg.get_scope(), &new_scopes);
            return Ok(());
        }

        self.db.put::<ScopeMsgIdSchema>(msg.get_scope(), &vec![id]);
        Ok(())
    }

    fn delete_msgs_before_timestamp(&self, msg_id: u128) -> DbResult<()> {
        let ids = self.get_msg_ids_before_timestamp(msg_id)?;

        let mut batch = SchemaBatch::new();
        for id in ids {
            batch.delete::<BridgeMsgIdSchema>(&id)?;
        }

        self.db.write_schemas(batch)?;
        Ok(())
    }

    fn get_msgs_by_scope(&self, scope: &[u8]) -> DbResult<Vec<BridgeMessage>> {
        let mut msg_scope = Scope::from_raw(scope).ok();

        if let Some(scope) = msg_scope {
            let mut msg_ids = Vec::new();

            // Regular loop for filtering and mapping
            for msg in (self.db.iter::<ScopeMsgIdSchema>()?).flatten() {
                let (m_scope, id) = msg.into_tuple();
                if scope == m_scope {
                    msg_ids.push(id);
                }
            }

            let mut msgs = Vec::new();

            // Iterating over filtered message IDs to fetch messages
            for message_id_group in msg_ids {
                for message_id in message_id_group {
                    if let Ok(Some(message)) = self.db.get::<BridgeMsgIdSchema>(&message_id) {
                        msgs.push(message);
                    }
                }
            }
            return Ok(msgs);
        }

        Err(DbError::InvalidArgument)
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use alpen_express_bridge_msg::types::BridgeMessage;
    use alpen_express_primitives::l1::L1TxProof;
    use alpen_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    fn setup_db() -> BridgeMsgDb {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        BridgeMsgDb::new(db, db_ops)
    }

    fn new_bridge_msg() -> (u128, BridgeMessage) {
        let arb = ArbitraryGenerator::new();

        let msg: BridgeMessage = arb.generate();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();

        (timestamp, msg)
    }

    #[test]
    fn test_write_msgs() {
        let br_db = setup_db();
        let (timestamp, msg) = new_bridge_msg();

        let result = br_db.write_msg(timestamp, msg);
        assert!(result.is_ok());
    }
    #[test]
    fn test_get_msg_ids_before_timestamp() {
        let br_db = setup_db();
        let (timestamp1, msg1) = new_bridge_msg();
        let (timestamp2, _) = new_bridge_msg();
        let (timestamp3, msg2) = new_bridge_msg();

        // Write messages to the database
        br_db.write_msg(timestamp1, msg1).unwrap();
        br_db.write_msg(timestamp3, msg2).unwrap();

        // Retrieve message IDs before the second timestamp
        let result = br_db.get_msg_ids_before_timestamp(timestamp2);
        assert!(result.is_ok());

        let ids = result.unwrap();
        assert!(ids.contains(&timestamp1));
    }

    #[test]
    fn test_delete_msgs_before_timestamp() {
        let br_db = setup_db();
        let (timestamp1, msg1) = new_bridge_msg();
        let (timestamp2, msg2) = new_bridge_msg();

        // Write messages to the database
        br_db.write_msg(timestamp1, msg1).unwrap();
        br_db.write_msg(timestamp2, msg2).unwrap();
        // Delete messages before the second timestamp
        let result = br_db.delete_msgs_before_timestamp(timestamp2);
        assert!(result.is_ok());

        // Check if only the second message remains
        let ids = br_db.get_msg_ids_before_timestamp(u128::MAX).unwrap();
        assert!(!ids.contains(&timestamp1));
    }

    #[test]
    fn test_get_msgs_by_scope() {
        let br_db = setup_db();
        let (timestamp1, mut msg1) = new_bridge_msg();
        let (timestamp2, mut msg2) = new_bridge_msg();

        let scope = msg1.get_scope_raw().unwrap();

        // Write messages to the database
        br_db.write_msg(timestamp1, msg1.clone()).unwrap();
        br_db.write_msg(timestamp2, msg2.clone()).unwrap();

        // Retrieve messages by scope
        let result = br_db.get_msgs_by_scope(&scope);
        assert!(result.is_ok());

        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_no_messages_for_nonexistent_scope() {
        let br_db = setup_db();
        let (timestamp, msg) = new_bridge_msg();

        // Write message to the database
        br_db.write_msg(timestamp, msg).unwrap();

        // Try to retrieve messages with a different scope
        let result = br_db.get_msgs_by_scope(&[1, 1, 1]);
        assert!(result.is_err());

        // Try to retrieve messages with a different scope
        let result = br_db.get_msgs_by_scope(&[0, 10, 0, 0, 0]);
        assert!(result.is_ok());

        // Should be empty since no message has the scope [1, 1, 1]
        let msgs = result.unwrap();
        assert!(msgs.is_empty());
    }
}
