#[macro_export]
macro_rules! impl_buf_wrapper {
    ($wrapper:ident, $name:ident, $len:expr) => {
        impl ::std::convert::From<$name> for $wrapper {
            fn from(value: $name) -> Self {
                Self(value)
            }
        }

        impl ::std::convert::From<$wrapper> for $name {
            fn from(value: $wrapper) -> Self {
                value.0
            }
        }

        impl ::std::convert::AsRef<[u8; $len]> for $wrapper {
            fn as_ref(&self) -> &[u8; $len] {
                self.0.as_ref()
            }
        }

        impl ::core::fmt::Debug for $wrapper {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Debug::fmt(&self.0, f)
            }
        }

        impl ::core::fmt::Display for $wrapper {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

pub mod internal {
    // Crate-internal impls.

    macro_rules! impl_buf_common {
        ($name:ident, $len:expr) => {
            impl $name {
                pub const LEN: usize = $len;

                pub fn new(data: [u8; $len]) -> Self {
                    Self(data)
                }

                pub fn as_slice(&self) -> &[u8] {
                    &self.0
                }

                pub fn as_mut_slice(&mut self) -> &mut [u8] {
                    &mut self.0
                }

                pub fn as_bytes(&self) -> &[u8] {
                    self.0.as_slice()
                }

                pub fn zero() -> Self {
                    Self([0; $len].into())
                }

                pub fn is_zero(&self) -> bool {
                    self.0.iter().all(|v| *v == 0)
                }
            }

            impl ::std::convert::AsRef<[u8; $len]> for $name {
                fn as_ref(&self) -> &[u8; $len] {
                    &self.0
                }
            }

            impl ::std::convert::AsMut<[u8]> for $name {
                fn as_mut(&mut self) -> &mut [u8] {
                    &mut self.0
                }
            }

            impl ::std::convert::From<[u8; $len]> for $name {
                fn from(data: [u8; $len]) -> Self {
                    Self(data)
                }
            }

            impl ::std::convert::From<$name> for [u8; $len] {
                fn from(buf: $name) -> Self {
                    buf.0
                }
            }

            impl<'a> ::std::convert::From<&'a [u8; $len]> for $name {
                fn from(data: &'a [u8; $len]) -> Self {
                    Self(*data)
                }
            }

            impl<'a> ::std::convert::TryFrom<&'a [u8]> for $name {
                type Error = &'a [u8];

                fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
                    if value.len() == $len {
                        let mut arr = [0; $len];
                        arr.copy_from_slice(value);
                        Ok(Self(arr))
                    } else {
                        Err(value)
                    }
                }
            }

            impl ::std::convert::From<$name>
                for ::reth_primitives::revm_primitives::alloy_primitives::FixedBytes<$len>
            {
                fn from(value: $name) -> Self {
                    value.0.into()
                }
            }

            impl
                ::std::convert::From<
                    ::reth_primitives::revm_primitives::alloy_primitives::FixedBytes<$len>,
                > for $name
            {
                fn from(
                    value: ::reth_primitives::revm_primitives::alloy_primitives::FixedBytes<$len>,
                ) -> Self {
                    value.0.into()
                }
            }

            impl ::std::default::Default for $name {
                fn default() -> Self {
                    Self([0; $len])
                }
            }

            impl ::std::fmt::Debug for $name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    // twice as large, required by the hex::encode_to_slice.
                    let mut buf = [0; $len * 2];
                    hex::encode_to_slice(self.0, &mut buf).expect("buf: enc hex");
                    f.write_str(unsafe { ::core::str::from_utf8_unchecked(&buf) })
                }
            }

            impl ::std::fmt::Display for $name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    // fmt only first and last bits of data.
                    let mut buf = [0; 6];
                    hex::encode_to_slice(&self.0[..3], &mut buf).expect("buf: enc hex");
                    f.write_str(unsafe { ::core::str::from_utf8_unchecked(&buf) })?;
                    f.write_str("..")?;
                    hex::encode_to_slice(&self.0[$len - 3..], &mut buf).expect("buf: enc hex");
                    f.write_str(unsafe { ::core::str::from_utf8_unchecked(&buf) })?;
                    Ok(())
                }
            }

            impl ::borsh::BorshSerialize for $name {
                fn serialize<W: ::std::io::Write>(&self, writer: &mut W) -> ::std::io::Result<()> {
                    let bytes = self.0.as_ref();
                    let _ = writer.write(bytes)?;
                    Ok(())
                }
            }

            impl ::borsh::BorshDeserialize for $name {
                fn deserialize_reader<R: ::std::io::Read>(
                    reader: &mut R,
                ) -> ::std::io::Result<Self> {
                    let mut array = [0u8; $len];
                    reader.read_exact(&mut array)?;
                    Ok(array.into())
                }
            }

            impl<'a> ::arbitrary::Arbitrary<'a> for $name {
                fn arbitrary(u: &mut ::arbitrary::Unstructured<'a>) -> ::arbitrary::Result<Self> {
                    let mut array = [0u8; $len];
                    u.fill_buffer(&mut array)?;
                    Ok(array.into())
                }
            }
        };
    }

    macro_rules! impl_buf_serde {
        // Historically, the Buf* types were wrapping FixedBytes.
        // Delegate serde to FixedBytes for now to not break anything.
        // TODO (STR-453): rework serde.
        ($name:ident, $len:expr) => {
            impl ::serde::Serialize for $name {
                #[inline]
                fn serialize<S: ::serde::Serializer>(
                    &self,
                    serializer: S,
                ) -> Result<S::Ok, S::Error> {
                    ::serde::Serialize::serialize(&::reth_primitives::revm_primitives::alloy_primitives::FixedBytes::<$len>::from(&self.0), serializer)
                }
            }

            impl<'de> ::serde::Deserialize<'de> for $name {
                #[inline]
                fn deserialize<D: ::serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> Result<Self, D::Error> {
                    ::serde::Deserialize::deserialize(deserializer)
                        .map(|v: ::reth_primitives::revm_primitives::alloy_primitives::FixedBytes<$len>| Self::from(v))
                }
            }
        };
    }

    pub(crate) use impl_buf_common;
    pub(crate) use impl_buf_serde;
}

#[cfg(test)]
mod tests {
    pub struct TestBuf20([u8; 20]);
    crate::macros::internal::impl_buf_common!(TestBuf20, 20);

    #[test]
    fn test_from_into_array() {
        let buf = TestBuf20::new([5u8; 20]);
        let arr: [u8; 20] = buf.into();
        assert_eq!(arr, [5; 20]);
    }

    #[test]
    fn test_from_array_ref() {
        let arr = [2u8; 20];
        let buf: TestBuf20 = TestBuf20::from(&arr);
        assert_eq!(buf.as_slice(), &arr);
    }

    #[test]
    fn test_default() {
        let buf = TestBuf20::default();
        assert_eq!(buf.as_slice(), &[0; 20]);
    }
}
