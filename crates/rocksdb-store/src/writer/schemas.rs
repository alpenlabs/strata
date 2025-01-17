use strata_db::types::{IntentEntry, PayloadEntry};
use strata_primitives::buf::Buf32;

use crate::{
    define_table_with_default_codec, define_table_with_seek_key_codec, define_table_without_codec,
    impl_borsh_value_codec,
};

define_table_with_seek_key_codec!(
    /// A table to store idx-> payload entry mapping
    (PayloadSchema) u64 => PayloadEntry
);

define_table_with_default_codec!(
    /// A table to store intentid -> intent mapping
    (IntentSchema) Buf32 => IntentEntry
);

define_table_with_seek_key_codec!(
    /// A table to store idx-> intent id mapping
    (IntentIdxSchema) u64 => Buf32
);
