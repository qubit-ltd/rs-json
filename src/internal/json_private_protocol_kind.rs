// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Identifies serde_json private compound protocols used by JSON serialization.

use crate::encode::JsonSerializerStateError;
use crate::internal::serde_json_compat::SERDE_JSON_NUMBER_TOKEN;
use crate::internal::serde_json_compat::SERDE_JSON_RAW_VALUE_TOKEN;

/// A private serde_json protocol recognized by the JSON encoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsonPrivateProtocolKind {
    /// Raw JSON value payload protocol.
    RawValue,
    /// Number lexical payload protocol.
    Number,
}

impl JsonPrivateProtocolKind {
    /// Returns the protocol kind for an exact serde_json private struct name.
    ///
    /// Only the private RawValue and Number protocol tokens are recognized;
    /// every other name is treated as an unrelated serializer struct.
    ///
    /// # Returns
    ///
    /// `Some` identifies a recognized private protocol, and `None` means the
    /// name does not exactly match either protocol token.
    #[inline]
    pub(crate) fn from_struct_name(name: &str) -> Option<Self> {
        match name {
            SERDE_JSON_RAW_VALUE_TOKEN => Some(Self::RawValue),
            SERDE_JSON_NUMBER_TOKEN => Some(Self::Number),
            _ => None,
        }
    }

    /// Returns the exact private struct name for this protocol.
    #[must_use]
    #[inline]
    pub(crate) const fn struct_name(self) -> &'static str {
        match self {
            Self::RawValue => SERDE_JSON_RAW_VALUE_TOKEN,
            Self::Number => SERDE_JSON_NUMBER_TOKEN,
        }
    }

    /// Returns the exact private payload field name for this protocol.
    #[must_use]
    #[inline]
    pub(crate) const fn field_name(self) -> &'static str {
        self.struct_name()
    }

    /// Returns the stable protocol failure for this kind.
    #[inline]
    pub(crate) const fn invalid_state(self) -> JsonSerializerStateError {
        match self {
            Self::RawValue => JsonSerializerStateError::InvalidRawValueProtocol,
            Self::Number => JsonSerializerStateError::InvalidNumberProtocol,
        }
    }
}
