use crate::{define_table_with_seek_key_codec, define_table_without_codec, impl_borsh_value_codec};

// todo: use proper types after they are defined on state crate
define_table_with_seek_key_codec!(
    /// A table to store idx-> blobid mapping
    (ProverTaskSchema) u64 => Vec<u8>
);
