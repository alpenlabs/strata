// Copied from sov-sdk's sov-db crate
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

#[macro_export]
macro_rules! impl_borsh_value_codec {
    ($table_name:ident, $value:ty) => {
        impl ::rockbound::schema::ValueCodec<$table_name> for $value {
            fn encode_value(
                &self,
            ) -> ::std::result::Result<::std::vec::Vec<u8>, ::rockbound::CodecError> {
                ::borsh::to_vec(self).map_err(Into::into)
            }

            fn decode_value(data: &[u8]) -> ::std::result::Result<Self, ::rockbound::CodecError> {
                ::borsh::BorshDeserialize::deserialize_reader(&mut &data[..]).map_err(Into::into)
            }
        }
    };
}

#[macro_export]
macro_rules! define_table_with_default_codec {
    ($(#[$docs:meta])+ ($table_name:ident) $key:ty => $value:ty) => {
        define_table_without_codec!($(#[$docs])+ ( $table_name ) $key => $value);

        impl ::rockbound::schema::KeyEncoder<$table_name> for $key {
            fn encode_key(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::rockbound::CodecError> {
                ::borsh::to_vec(self).map_err(Into::into)
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
