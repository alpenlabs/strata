use alpen_express_bridge_msg::types::{BridgeMessage, Scope};

use crate::{
    define_table_with_default_codec, define_table_with_seek_key_codec, define_table_without_codec,
    impl_borsh_value_codec,
};

define_table_with_seek_key_codec!(
    /// A table to store mapping of Unix epoch to Bridge Message
    (BridgeMsgIdSchema) u128 => BridgeMessage
);

define_table_with_default_codec!(
    /// A table to store mapping of scope to Bridge Message Ids
    (ScopeMsgIdSchema) Vec<u8> => Vec<u128>
);
