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

        impl Into<[u8; $len]> for $name {
            fn into(self) -> [u8; $len] {
                self.0
            }
        }

        // Add any other common impls here if needed.
    };
}
