//! Middleware loans whose publication follows the managed publisher's state.

use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;

use super::Activation;
use crate::{LoanedMessage, Result, WrappedTypesupport};

/// Native middleware storage borrowed from a lifecycle publisher.
///
/// Dereferencing exposes the native message fields for filling in place. Its
/// [`Self::publish`] method checks the publisher's current activation state;
/// deactivated publication returns the loan without sending it to DDS. The
/// owning node stays mutably borrowed until this loan is published or dropped,
/// just as with [`LoanedMessage`].
pub struct LifecycleLoanedMessage<'node, T: WrappedTypesupport> {
    message: LoanedMessage<'node, T>,
    enabled: Activation,
}

impl<'node, T: WrappedTypesupport> LifecycleLoanedMessage<'node, T> {
    pub(super) fn new(message: LoanedMessage<'node, T>, enabled: Activation) -> Self {
        Self { message, enabled }
    }

    /// Publish while activated, otherwise return the loan to the middleware.
    ///
    /// Returns `Ok(())` for a gated drop. Activation is checked at publication,
    /// rather than when storage is borrowed. A concurrent deactivation does
    /// not retract a publication that has already observed activation.
    pub fn publish(self) -> Result<()> {
        if self.enabled.load(Ordering::SeqCst) {
            self.message.publish()
        } else {
            drop(self.message);
            Ok(())
        }
    }
}

impl<T: WrappedTypesupport> Deref for LifecycleLoanedMessage<'_, T> {
    type Target = T::CStruct;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl<T: WrappedTypesupport> DerefMut for LifecycleLoanedMessage<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.message
    }
}
