//! Message related types.

use std::any::Any;

use crate::SubprotocolId;

/// Generic wrapper around a inter-proto msg.
pub trait InterprotoMsg: Any + 'static {
    /// Returns the ID of the subprotocol that produced this messages.
    fn id(&self) -> SubprotocolId;

    /// Converts the message into a `Box<dyn Any>` for upcasting.
    ///
    /// The impl of this function should always be `Box::new(self)`.  For
    /// technical type system reasons, this cannot be provided as a default
    /// impl.
    ///
    /// This can be removed by using trait upcasting in Rust 1.86.
    fn to_box_any(&self) -> Box<dyn Any>;
}

/// Empty impl that can't be constructed.
#[derive(Copy, Clone, Debug)]
pub struct NullMsg<const ID: SubprotocolId>;

impl<const ID: SubprotocolId> InterprotoMsg for NullMsg<ID> {
    fn id(&self) -> SubprotocolId {
        ID
    }

    fn to_box_any(&self) -> Box<dyn Any> {
        Box::new(self.clone())
    }
}

/// Stub type for SPS-msg-fmt log.
///
/// This should be converted to be a wrapper from the strata-common repo.
pub struct Log {
    ty: u16,
    body: Vec<u8>,
}

impl Log {
    pub fn new(ty: u16, body: Vec<u8>) -> Self {
        Self { ty, body }
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;

    use super::InterprotoMsg;
    use crate::SubprotocolId;

    #[derive(Clone)]
    struct Foo {
        x: u32,
    }

    impl InterprotoMsg for Foo {
        fn id(&self) -> SubprotocolId {
            42
        }

        fn to_box_any(&self) -> Box<dyn Any> {
            Box::new(self.clone())
        }
    }

    fn test() {
        // TODO
        let inst = Foo { x: 5 };

        let inst_box = Box::new(inst) as Box<dyn InterprotoMsg>;
    }
}
