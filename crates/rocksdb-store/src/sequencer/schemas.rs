use strata_db::types::PayloadEntry;
use strata_primitives::buf::Buf32;

use crate::{
    define_table_with_default_codec, define_table_with_seek_key_codec, define_table_without_codec,
    impl_borsh_value_codec,
};

define_table_with_seek_key_codec!(
    /// A table to store idx-> blobid mapping
    (SeqBlobIdSchema) u64 => Buf32
);

define_table_with_default_codec!(
    /// A table to store blobid -> blob mapping
    (SeqBlobSchema) Buf32 => PayloadEntry
);
