// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private normalization pipeline for lenient JSON decoding.

use std::borrow::Cow;

use qubit_budget::ResourceBudget;
use qubit_budget::json::JsonDecodeAttempt;
use qubit_budget::json::JsonResource;

use super::super::NormalizingJsonDecodeError;
use super::super::NormalizingJsonDecodeOptions;
use super::control_character_escaper::ControlCharacterEscaper;
use super::markdown_fence::MarkdownFence;

/// Normalizes one raw JSON text input before JSON parsing.
#[derive(Debug, Clone)]
pub(crate) struct JsonNormalizer {
    /// Stores the option set used by the normalizer.
    options: NormalizingJsonDecodeOptions,
}

impl Default for JsonNormalizer {
    /// Creates a normalizer using the default lenient option set.
    ///
    /// # Returns
    ///
    /// A normalizer configured with [`NormalizingJsonDecodeOptions::default`].
    #[inline(always)]
    fn default() -> Self {
        Self::new(NormalizingJsonDecodeOptions::default())
    }
}

impl JsonNormalizer {
    /// Creates a normalizer with the provided lenient decoding options.
    ///
    /// # Parameters
    ///
    /// * `options` - Immutable normalization and error-diagnostic options.
    ///
    /// # Returns
    ///
    /// A normalizer configured with `options`.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn new(options: NormalizingJsonDecodeOptions) -> Self {
        Self { options }
    }

    /// Returns the configuration used by this normalizer.
    ///
    /// # Returns
    ///
    /// The immutable normalization options.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn options(&self) -> &NormalizingJsonDecodeOptions {
        &self.options
    }

    /// Normalizes one raw JSON text input into text ready for parsing.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON text to normalize.
    ///
    /// # Returns
    ///
    /// A borrowed view when normalization can be represented as a slice of the
    /// original input, or owned text when control characters require escaping.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizingJsonDecodeError`] when the raw or normalized input
    /// exceeds its configured limit or becomes empty at a normalization
    /// boundary.
    pub(crate) fn normalize<'a>(
        &self,
        input: &'a str,
        attempt: &mut JsonDecodeAttempt<'_, JsonResource, usize>,
    ) -> Result<Cow<'a, str>, NormalizingJsonDecodeError> {
        self.normalize_with_attempt(input, attempt, true)
    }

    /// Normalizes input after a caller has already charged its raw bytes.
    pub(crate) fn normalize_after_raw_charge<'a>(
        &self,
        input: &'a str,
        attempt: &mut JsonDecodeAttempt<'_, JsonResource, usize>,
    ) -> Result<Cow<'a, str>, NormalizingJsonDecodeError> {
        self.normalize_with_attempt(input, attempt, false)
    }

    /// Runs normalization while charging raw and normalized input budgets.
    fn normalize_with_attempt<'a>(
        &self,
        input: &'a str,
        attempt: &mut JsonDecodeAttempt<'_, JsonResource, usize>,
        charge_raw_input: bool,
    ) -> Result<Cow<'a, str>, NormalizingJsonDecodeError> {
        let raw_input_bytes = input.len();
        if charge_raw_input {
            self.consume_raw_input(attempt, raw_input_bytes)?;
        }
        let input = self.require_non_empty(input, raw_input_bytes)?;
        // Keep strict decoding on this shared pipeline: disabled stages return
        // the input unchanged, while a dedicated bypass added option checks
        // without a stable A/B benefit for downstream-sized inputs.
        let input = self.trim_if_enabled(input);
        let input = self.strip_utf8_bom(input);
        let input = self.trim_if_enabled(input);
        let input = MarkdownFence::strip_outer(
            input,
            self.options.markdown_fence_policy(),
        );
        let input = self.trim_if_enabled(input);
        let (normalized_len, needs_escape) = self.scan_normalized_size(input);
        self.consume_normalized_input(
            attempt,
            raw_input_bytes,
            normalized_len,
        )?;
        let input = ControlCharacterEscaper::escape_with_scan(
            input,
            normalized_len,
            needs_escape,
        );

        if input.is_empty() {
            Err(NormalizingJsonDecodeError::empty_input(
                raw_input_bytes,
                Some(input.len()),
                self.options.diagnostic_policy(),
            ))
        } else {
            Ok(input)
        }
    }

    /// Rejects empty text according to the configured whitespace policy.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw input to check.
    /// * `raw_input_bytes` - Original raw input length in bytes.
    ///
    /// # Returns
    ///
    /// The unchanged input when it is accepted.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizingJsonDecodeError`] when the input is empty under the
    /// active whitespace policy.
    fn require_non_empty<'a>(
        &self,
        input: &'a str,
        raw_input_bytes: usize,
    ) -> Result<&'a str, NormalizingJsonDecodeError> {
        if self.options.trim_whitespace() {
            if input.trim().is_empty() {
                return Err(NormalizingJsonDecodeError::empty_input(
                    raw_input_bytes,
                    None,
                    self.options.diagnostic_policy(),
                ));
            }
        } else if input.is_empty() {
            return Err(NormalizingJsonDecodeError::empty_input(
                raw_input_bytes,
                None,
                self.options.diagnostic_policy(),
            ));
        }
        Ok(input)
    }

    /// Scans normalized input before allocating repaired text.
    ///
    /// # Parameters
    ///
    /// * `input` - Text after non-allocating normalization steps.
    /// * `input` - Text after non-allocating normalization steps.
    ///
    /// # Returns
    ///
    /// The normalized byte length and whether control-character escaping is
    /// required.
    fn scan_normalized_size(&self, input: &str) -> (usize, bool) {
        ControlCharacterEscaper::scan(
            input,
            self.options.escape_control_chars_in_strings(),
        )
    }

    /// Charges raw input bytes and maps a rejected budget to the stable error.
    fn consume_raw_input(
        &self,
        attempt: &mut JsonDecodeAttempt<'_, JsonResource, usize>,
        amount: usize,
    ) -> Result<(), NormalizingJsonDecodeError> {
        if attempt.try_consume_input_bytes(amount).is_err() {
            return Err(NormalizingJsonDecodeError::input_too_large(
                amount,
                attempt.input_budget().map_or(amount, ResourceBudget::limit),
                self.options.diagnostic_policy(),
            ));
        }
        Ok(())
    }

    /// Charges normalized input bytes before allocating escaped text.
    fn consume_normalized_input(
        &self,
        attempt: &mut JsonDecodeAttempt<'_, JsonResource, usize>,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Result<(), NormalizingJsonDecodeError> {
        if attempt
            .try_consume_normalized_input_bytes(normalized_input_bytes)
            .is_err()
        {
            return Err(
                NormalizingJsonDecodeError::normalized_input_too_large(
                    raw_input_bytes,
                    normalized_input_bytes,
                    attempt
                        .normalized_input_budget()
                        .map_or(normalized_input_bytes, ResourceBudget::limit),
                    self.options.diagnostic_policy(),
                ),
            );
        }
        Ok(())
    }

    /// Trims a borrowed input slice when trimming is enabled.
    ///
    /// # Parameters
    ///
    /// * `input` - Borrowed text to conditionally trim.
    ///
    /// # Returns
    ///
    /// A borrowed view of the trimmed or unchanged input.
    #[inline]
    #[must_use]
    fn trim_if_enabled<'a>(&self, input: &'a str) -> &'a str {
        if self.options.trim_whitespace() {
            input.trim()
        } else {
            input
        }
    }

    /// Removes one leading UTF-8 byte order mark when configured.
    ///
    /// # Parameters
    ///
    /// * `input` - Text to inspect for a leading byte order mark.
    ///
    /// # Returns
    ///
    /// A borrowed view with one leading mark removed when configured, or the
    /// unchanged input.
    #[inline]
    #[must_use]
    fn strip_utf8_bom<'a>(&self, input: &'a str) -> &'a str {
        if self.options.strip_utf8_bom() {
            input.strip_prefix('\u{feff}').unwrap_or(input)
        } else {
            input
        }
    }
}
