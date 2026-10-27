use crate::{
    msg_types::{SerializedMessageBuffer, WrappedNativeMsg},
    Error, Result, WrappedTypesupport,
};

/// Reusable native message and serialized-byte storage for one ROS message type.
///
/// `serialize` returns bytes borrowed from this serializer. Rust prevents another serialization
/// while the previous borrow is still used. Call `to_vec()` on the returned slice when independent
/// ownership is needed. Native fields reuse capacity; serialized storage is owned by this
/// serializer and may be resized by the selected RMW implementation.
///
/// The serializer can move between threads and each thread may own its own serializer.
/// Allocation, conversion and middleware errors are returned as [`Error`].
pub struct MessageSerializer<T: WrappedTypesupport> {
    native: WrappedNativeMsg<T>,
    buffer: SerializedMessageBuffer,
}

impl<T: WrappedTypesupport> MessageSerializer<T> {
    /// Creates an initialized native message and an initially empty serialized buffer.
    pub fn new() -> Result<Self> {
        Self::with_capacity(0)
    }

    /// Creates a serializer with initially at least `capacity` bytes of serialized storage.
    ///
    /// Capacity for native message fields is established by serialization inputs. An RMW may
    /// resize or replace the serialized buffer even when its initial capacity is sufficient.
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        let native = WrappedNativeMsg::try_new()?;
        let buffer =
            SerializedMessageBuffer::with_capacity(capacity).map_err(Error::from_rcl_error)?;
        Ok(Self { native, buffer })
    }

    /// Serializes into reusable storage and borrows the resulting ROS wire bytes.
    ///
    /// Borrowed bytes remain valid until this serializer is mutated or dropped. A failure returns
    /// no byte view; a subsequent valid input may be serialized again. No middleware-side borrowed
    /// message or deserialization state is retained.
    pub fn serialize(&mut self, message: &T) -> Result<&[u8]> {
        message.try_copy_to_native(&mut self.native)?;
        self.buffer.serialize(&self.native)
    }

    /// Returns the currently retained serialized-buffer capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
