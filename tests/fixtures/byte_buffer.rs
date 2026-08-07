// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the byte-oriented target used by decoder regression tests.

use serde::Deserialize;
use serde::Deserializer;

use super::internal::ByteBufferVisitor;

/// Byte-oriented target used to verify the decoder's UTF-8 boundary.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ByteBuffer(
    /// Stores the raw bytes accepted by the target deserializer.
    pub(crate) Vec<u8>,
);

impl<'de> Deserialize<'de> for ByteBuffer {
    /// Requests Serde's byte-buffer path for the target value.
    ///
    /// # Parameters
    ///
    /// * `deserializer` - Deserializer that provides the byte-buffer value.
    ///
    /// # Returns
    ///
    /// The owned byte-buffer fixture.
    ///
    /// # Errors
    ///
    /// Returns the deserializer error when the input cannot be represented as
    /// a byte buffer.
    #[inline(always)]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_byte_buf(ByteBufferVisitor)
    }
}
