use alpen_express_primitives::relay::types::BridgeMessage;

use crate::DbResult;

/// Interface for storing and retrieving bridge messages.
#[cfg_attr(feature = "mocks", automock)]
pub trait BridgeMessageDb {
    /// Stores a bridge message
    fn write_msg(&self, id: u128, msg: BridgeMessage) -> DbResult<()>;

    /// Deletes messages by their UNIX epoch.
    fn delete_msgs_before_timestamp(&self, msg_ids: u128) -> DbResult<()>;

    /// Retrieves messages by their scope.
    fn get_msgs_by_scope(&self, scope: &[u8]) -> DbResult<Vec<BridgeMessage>>;
}
