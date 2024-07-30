use alpen_express_primitives::buf::Buf32;

use crate::define_table_with_default_codec;
use crate::define_table_with_seek_key_codec;
use crate::define_table_without_codec;
use crate::impl_borsh_value_codec;
use alpen_express_db::types::{BlobEntry, TxEntry};

define_table_with_default_codec!(
    /// A table to store L1 txns
    (SeqL1TxnSchema) Buf32 => Vec<u8>
);

define_table_with_seek_key_codec!(
    /// A table to store mapping of idx to L1 tx
    (SeqL1TxIdSchema) u64 => Buf32
);

define_table_with_seek_key_codec!(
    /// A table to store idx-> blobid mapping
    (SeqBlobIdSchema) u64 => Buf32
);

define_table_with_default_codec!(
    /// A table to store blobid -> blob mapping
    (SeqBlobSchema) Buf32 => BlobEntry
);
