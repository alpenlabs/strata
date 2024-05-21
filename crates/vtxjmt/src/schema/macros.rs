//! This module defines the following tables:
//!
//!
//! Slot Tables:
//! - `SlotNumber -> StoredSlot`
//! - `SlotNumber -> Vec<BatchNumber>`
//!
//! Batch Tables:
//! - `BatchNumber -> StoredBatch`
//! - `BatchHash -> BatchNumber`
//!
//! Tx Tables:
//! - `TxNumber -> (TxHash,Tx)`
//! - `TxHash -> TxNumber`
//!
//! Event Tables:
//! - `(EventKey, TxNumber) -> EventNumber`
//! - `EventNumber -> (EventKey, EventValue)`
//!
//! JMT Tables:
//! - `KeyHash -> Key`
//! - `(Key, Version) -> JmtValue`
//! - `NodeKey -> Node`
//!
//! Module Accessory State Table:
//! - `(ModuleIdBytes, Key) -> Value`

/// Macro to define a table that implements [`rockbound::Schema`].
/// `KeyCodec<Schema>` and `ValueCodec<Schema>` must be implemented separately.
///
/// ```ignore
/// define_table_without_codec!(
///  /// A table storing keys and value
///  (MyTable) MyKey => MyValue
/// )
///
/// // This impl must be written by hand
/// impl KeyCodec<MyTable> for MyKey {
/// // ...
/// }
///
/// // This impl must be written by hand
/// impl ValueCodec<MyTable> for MyValue {
/// // ...
/// }
/// ```
#[macro_export]
macro_rules! define_table_without_codec {
    ($(#[$docs:meta])+ ( $table_name:ident ) $key:ty => $value:ty) => {
        $(#[$docs])+
        ///
        #[doc = concat!("Takes [`", stringify!($key), "`] as a key and returns [`", stringify!($value), "`]")]
        #[derive(Clone, Copy, Debug, Default)]
        pub(crate) struct $table_name;

        impl ::rockbound::schema::Schema for $table_name {
            const COLUMN_FAMILY_NAME: &'static str = $table_name::table_name();
            type Key = $key;
            type Value = $value;
        }

        impl $table_name {
            #[doc=concat!("Return ", stringify!($table_name), " as it is present inside the database.")]
            pub const fn table_name() -> &'static str {
                ::core::stringify!($table_name)
            }
        }

        impl ::std::fmt::Display for $table_name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::core::write!(f, "{}", stringify!($table_name))
            }
        }
    };
}

/// Implements [`::rockbound::schema::KeyEncoder`] for a given type using Borsh.
#[macro_export]
macro_rules! impl_borsh_value_codec {
    ($table_name:ident, $value:ty) => {
        impl ::rockbound::schema::ValueCodec<$table_name> for $value {
            fn encode_value(
                &self,
            ) -> ::std::result::Result<::std::vec::Vec<u8>, ::rockbound::CodecError> {
                ::borsh::BorshSerialize::try_to_vec(self).map_err(Into::into)
            }

            fn decode_value(data: &[u8]) -> ::std::result::Result<Self, ::rockbound::CodecError> {
                ::borsh::BorshDeserialize::deserialize_reader(&mut &data[..]).map_err(Into::into)
            }
        }
    };
}

/// Macro to define a table that implements [`rockbound::schema::Schema`].
/// Automatically generates KeyCodec<...> and ValueCodec<...> implementations
/// using the Encode and Decode traits from sov_rollup_interface
///
/// ```ignore
/// define_table_with_default_codec!(
///  /// A table storing keys and value
///  (MyTable) MyKey => MyValue
/// )
/// ```
#[macro_export]
macro_rules! define_table_with_default_codec {
    ($(#[$docs:meta])+ ($table_name:ident) $key:ty => $value:ty) => {
        define_table_without_codec!($(#[$docs])+ ( $table_name ) $key => $value);

        impl ::rockbound::schema::KeyEncoder<$table_name> for $key {
            fn encode_key(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::rockbound::CodecError> {
                ::borsh::BorshSerialize::try_to_vec(self).map_err(Into::into)
            }
        }

        impl ::rockbound::schema::KeyDecoder<$table_name> for $key {
            fn decode_key(data: &[u8]) -> ::std::result::Result<Self, ::rockbound::CodecError> {
                ::borsh::BorshDeserialize::deserialize_reader(&mut &data[..]).map_err(Into::into)
            }
        }

        impl_borsh_value_codec!($table_name, $value);
    };
}

/// Macro similar to [`define_table_with_default_codec`], but to be used when
/// your key type should be [`SeekKeyEncoder`]. Borsh serializes integers as
/// little-endian, but RocksDB uses lexicographic ordering which is only
/// compatible with big-endian, so we use [`bincode`] with the big-endian option
/// here.
#[macro_export]
macro_rules! define_table_with_seek_key_codec {
    ($(#[$docs:meta])+ ($table_name:ident) $key:ty => $value:ty) => {
        define_table_without_codec!($(#[$docs])+ ( $table_name ) $key => $value);

        impl ::rockbound::schema::KeyEncoder<$table_name> for $key {
            fn encode_key(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::rockbound::CodecError> {
                use ::anyhow::Context as _;
                use ::bincode::Options as _;

                let bincode_options = ::bincode::options()
                    .with_fixint_encoding()
                    .with_big_endian();

                bincode_options.serialize(self).context("Failed to serialize key").map_err(Into::into)
            }
        }

        impl ::rockbound::schema::KeyDecoder<$table_name> for $key {
            fn decode_key(data: &[u8]) -> ::std::result::Result<Self, ::rockbound::CodecError> {
                use ::anyhow::Context as _;
                use ::bincode::Options as _;

                let bincode_options = ::bincode::options()
                    .with_fixint_encoding()
                    .with_big_endian();

                bincode_options.deserialize_from(&mut &data[..]).context("Failed to deserialize key").map_err(Into::into)
            }
        }

        impl ::rockbound::SeekKeyEncoder<$table_name> for $key {
            fn encode_seek_key(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::rockbound::CodecError> {
                <Self as ::rockbound::schema::KeyEncoder<$table_name>>::encode_key(self)
            }
        }

        impl_borsh_value_codec!($table_name, $value);
    };
}