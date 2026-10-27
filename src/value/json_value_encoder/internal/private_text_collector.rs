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
pub(super) struct PrivateTextCollector<'a> {
    text: String,
    check_text_bytes: &'a mut dyn FnMut(usize) -> Result<(), JsonSerializationError>,
    failure: Option<JsonSerializationError>,
}

impl<'a> PrivateTextCollector<'a> {
    pub(super) fn new(
        check_text_bytes: &'a mut dyn FnMut(usize) -> Result<(), JsonSerializationError>,
    ) -> Self {
        Self {
            text: String::new(),
            check_text_bytes,
            failure: None,
        }
    }

    pub(super) fn take_failure(&mut self) -> Option<JsonSerializationError> {
        self.failure
    }

    pub(super) fn into_string(self) -> String {
        self.text
    }
}

impl fmt::Write for PrivateTextCollector<'_> {
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
