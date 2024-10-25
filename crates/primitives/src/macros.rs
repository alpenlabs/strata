macro_rules! impl_buf {
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
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }

        impl AsMut<[u8]> for $name {
            fn as_mut(&mut self) -> &mut [u8] {
                &mut self.0
            }
        }

        impl From<[u8; $len]> for $name {
            fn from(data: [u8; $len]) -> Self {
                Self(data)
            }
        }

        impl From<$name> for [u8; $len] {
            fn from(buf: $name) -> Self {
                buf.0
            }
        }

        impl<'a> From<&'a [u8; $len]> for $name {
            fn from(data: &'a [u8; $len]) -> Self {
                Self(*data)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self([0; $len])
            }
        }

        // Add any other common impls here if needed.
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Buf20([u8; 20]);
impl_buf!(Buf20, 20);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Buf32([u8; 32]);
impl_buf!(Buf32, 32);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Buf64([u8; 64]);
impl_buf!(Buf64, 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_array() {
        let arr = [1; 20];
        let buf: Buf20 = Buf20::from(arr);
        assert_eq!(buf.as_slice(), &arr);
    }

    #[test]
    fn test_from_array_ref() {
        let arr = [2; 20];
        let buf: Buf20 = Buf20::from(&arr);
        assert_eq!(buf.as_slice(), &arr);
    }

    #[test]
    fn test_as_ref_dereference() {
        let buf = Buf20::new([4; 20]);
        let arr: [u8; 20] = buf.as_ref().try_into().unwrap();
        assert_eq!(arr, [4; 20]);
    }

    #[test]
    fn test_default() {
        let buf = Buf20::default();
        assert_eq!(buf.as_slice(), &[0; 20]);
    }

    #[test]
    fn test_from_into_array() {
        let buf = Buf20::new([5; 20]);
        let arr: [u8; 20] = buf.into();
        assert_eq!(arr, [5; 20]);
    }
}
