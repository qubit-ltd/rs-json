// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private visitor for the byte-buffer test fixture.

use std::fmt;

use serde::de::{
    self,
    Visitor,
};

use crate::fixtures::ByteBuffer;

/// Visitor that accepts JSON strings through Serde's byte-buffer interface.
pub(crate) struct ByteBufferVisitor;

impl<'de> Visitor<'de> for ByteBufferVisitor {
    type Value = ByteBuffer;

    /// Describes the byte-string input accepted by this visitor.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Formatter that receives the expectation text.
    ///
    /// # Returns
    ///
    /// The formatter result.
    #[inline(always)]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON byte string")
    }

    /// Copies borrowed bytes into the owned test target.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed bytes supplied by the deserializer.
    ///
    /// # Returns
    ///
    /// The owned byte-buffer fixture.
    ///
    /// # Errors
    ///
    /// This implementation does not construct an error.
    #[inline]
    fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(ByteBuffer(value.to_vec()))
    }

    /// Copies transient bytes into the owned test target.
    ///
    /// # Parameters
    ///
    /// * `value` - Transient bytes supplied by the deserializer.
    ///
    /// # Returns
    ///
    /// The owned byte-buffer fixture.
    ///
    /// # Errors
    ///
    /// This implementation does not construct an error.
    #[inline]
    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(ByteBuffer(value.to_vec()))
    }

    /// Moves owned bytes into the test target.
    ///
    /// # Parameters
    ///
    /// * `value` - Owned bytes supplied by the deserializer.
    ///
    /// # Returns
    ///
    /// The owned byte-buffer fixture.
    ///
    /// # Errors
    ///
    /// This implementation does not construct an error.
    #[inline(always)]
    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(ByteBuffer(value))
    }
}
