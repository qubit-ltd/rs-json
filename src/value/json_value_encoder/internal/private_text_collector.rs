// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Bounded writer for text emitted by Serde's Display collection protocol.

use std::fmt;

use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;

/// Collects Display text only after each prospective byte length is accepted.
///
/// A rejected write is retained as a sticky failure. Callers must inspect that
/// failure before taking the collected prefix with [`Self::into_string`].
pub(super) struct PrivateTextCollector<'a> {
    /// Text accepted by the length check so far.
    text: String,
    /// Checks a prospective cumulative UTF-8 byte length before appending.
    check_text_bytes: &'a mut dyn FnMut(usize) -> Result<(), JsonSerializationError>,
    /// First failure, retained so later writes cannot replace its cause.
    failure: Option<JsonSerializationError>,
}

impl<'a> PrivateTextCollector<'a> {
    /// Creates an empty collector using the supplied cumulative-length check.
    ///
    /// # Parameters
    ///
    /// * `check_text_bytes` - Callback that accepts or rejects each prospective
    ///   cumulative UTF-8 byte length before text is appended.
    ///
    /// # Returns
    ///
    /// An empty collector that uses the callback for every write.
    pub(super) fn new(
        check_text_bytes: &'a mut dyn FnMut(usize) -> Result<(), JsonSerializationError>,
    ) -> Self {
        Self {
            text: String::new(),
            check_text_bytes,
            failure: None,
        }
    }

    /// Returns a copy of the retained failure without clearing it.
    ///
    /// # Returns
    ///
    /// `Some(error)` for the first rejected write, or `None` if every write
    /// has succeeded so far.
    pub(super) fn take_failure(&mut self) -> Option<JsonSerializationError> {
        self.failure
    }

    /// Returns the accepted text prefix; callers must first handle any failure.
    ///
    /// # Returns
    ///
    /// The text accepted by all successful writes. Any later write after the
    /// first failure is excluded.
    pub(super) fn into_string(self) -> String {
        self.text
    }
}

impl fmt::Write for PrivateTextCollector<'_> {
    /// Checks the prospective cumulative UTF-8 byte length before appending.
    ///
    /// Length overflow or rejection by the checker stores the first failure
    /// and returns `fmt::Error`. Once a failure is stored, every later write
    /// returns `fmt::Error` without invoking the checker or changing that
    /// failure. Text is appended only after the check succeeds.
    ///
    /// # Parameters
    ///
    /// * `text` - UTF-8 text to append after its prospective cumulative byte
    ///   length passes the checker.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the text is appended; otherwise, `Err(fmt::Error)`.
    ///
    /// # Errors
    ///
    /// Returns `fmt::Error` if the cumulative byte length overflows, the
    /// checker rejects the length, or an earlier write has failed.
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.failure.is_some() {
            return Err(fmt::Error);
        }
        let Some(length) = self.text.len().checked_add(text.len()) else {
            self.failure = Some(JsonSerializationError::new(
                JsonSerializationErrorKind::CustomSerialization,
            ));
            return Err(fmt::Error);
        };
        if let Err(error) = (self.check_text_bytes)(length) {
            self.failure = Some(error);
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}
