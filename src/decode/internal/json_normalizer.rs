// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private normalization pipeline for lenient JSON decoding.

use std::borrow::Cow;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonDecodeAttempt;

use super::super::JsonDecodeError;
use super::super::JsonDecodeStage;
use super::super::NormalizingJsonDecodePolicy;
use super::control_character_escaper::ControlCharacterEscaper;
use super::markdown_fence::MarkdownFence;

/// Normalizes one raw JSON text input before JSON parsing.
#[derive(Debug, Clone)]
pub(crate) struct JsonNormalizer {
    /// Stores the policy used by the normalizer.
    policy: NormalizingJsonDecodePolicy,
}

impl Default for JsonNormalizer {
    /// Creates a normalizer using the default lenient policy.
    ///
    /// # Returns
    ///
    /// A normalizer configured with the default lenient policy.
    #[inline(always)]
    fn default() -> Self {
        Self::new(NormalizingJsonDecodePolicy::default())
    }
}

impl JsonNormalizer {
    /// Creates a normalizer with the provided decoding policy.
    ///
    /// # Parameters
    ///
    /// * `policy` - Normalization and diagnostic behavior used by the pipeline.
    ///
    /// # Returns
    ///
    /// A normalizer that applies `policy` to each input.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn new(policy: NormalizingJsonDecodePolicy) -> Self {
        Self { policy }
    }

    /// Returns the immutable configuration used by this normalizer.
    ///
    /// # Returns
    ///
    /// A shared reference to the active normalization policy.
    #[inline(always)]
    #[must_use = "the normalization policy determines input handling"]
    pub(crate) const fn policy(&self) -> &NormalizingJsonDecodePolicy {
        &self.policy
    }

    /// Normalizes text while charging both raw and normalized input budgets.
    ///
    /// Returns borrowed text when non-allocating transformations suffice and
    /// owned text when control-character escaping is required. Raw and
    /// normalized input charges remain committed on any later failure.
    ///
    /// # Type Parameters
    ///
    /// * `'input` - Lifetime of the borrowed input and returned text.
    /// * `R` - Resource kind tracked by the decoding attempt.
    /// * `Q` - Quantity type used for resource accounting.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON-like text to normalize.
    /// * `attempt` - Attempt receiving raw and normalized input charges.
    ///
    /// # Returns
    ///
    /// Borrowed normalized text when possible, or owned text when escaping
    /// requires allocation.
    ///
    /// # Errors
    ///
    /// Returns an error when input is empty under the policy or either input
    /// charge exceeds the attempt's budget.
    pub(crate) fn normalize<'input, R, Q>(
        &self,
        input: &'input str,
        attempt: &mut JsonDecodeAttempt<'_, R, Q>,
    ) -> Result<Cow<'input, str>, JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        self.normalize_with_attempt(input, attempt, true)
    }

    /// Normalizes text after the caller has already charged raw input bytes.
    ///
    /// # Type Parameters
    ///
    /// * `'input` - Lifetime of the borrowed input and returned text.
    /// * `R` - Resource kind tracked by the decoding attempt.
    /// * `Q` - Quantity type used for resource accounting.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON-like text whose raw byte charge is already
    ///   recorded.
    /// * `attempt` - Attempt receiving the normalized input charge.
    ///
    /// # Returns
    ///
    /// Borrowed normalized text when possible, or owned text when escaping
    /// requires allocation.
    ///
    /// # Errors
    ///
    /// Returns an error when input is empty under the policy or the normalized
    /// input charge exceeds the attempt's budget.
    pub(crate) fn normalize_after_raw_charge<'input, R, Q>(
        &self,
        input: &'input str,
        attempt: &mut JsonDecodeAttempt<'_, R, Q>,
    ) -> Result<Cow<'input, str>, JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        self.normalize_with_attempt(input, attempt, false)
    }

    /// Runs the configured transformation pipeline and input accounting.
    ///
    /// # Type Parameters
    ///
    /// * `'input` - Lifetime of the borrowed input and returned text.
    /// * `R` - Resource kind tracked by the decoding attempt.
    /// * `Q` - Quantity type used for resource accounting.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw JSON-like text to normalize.
    /// * `attempt` - Attempt receiving input charges.
    /// * `charge_raw_input` - Whether this call records the raw byte charge.
    ///
    /// # Returns
    ///
    /// Borrowed normalized text when possible, or owned text when escaping
    /// requires allocation.
    ///
    /// # Errors
    ///
    /// Returns an error when input is empty under the policy or an input charge
    /// exceeds the attempt's budget.
    fn normalize_with_attempt<'input, R, Q>(
        &self,
        input: &'input str,
        attempt: &mut JsonDecodeAttempt<'_, R, Q>,
        charge_raw_input: bool,
    ) -> Result<Cow<'input, str>, JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        let raw_input_bytes = input.len();
        if charge_raw_input {
            self.consume_raw_input(attempt, raw_input_bytes)?;
        }
        let input = self.require_non_empty(input, raw_input_bytes)?;
        let input = self.trim_if_enabled(input);
        let input = self.strip_utf8_bom(input);
        let input = self.trim_if_enabled(input);
        let input = MarkdownFence::strip_outer(input, self.policy.markdown_fence_policy());
        let input = self.trim_if_enabled(input);
        let (normalized_len, needs_escape) = self.scan_normalized_size(input);
        self.consume_normalized_input(attempt, raw_input_bytes, normalized_len)?;
        let input = ControlCharacterEscaper::escape_with_scan(input, normalized_len, needs_escape);

        if input.is_empty() {
            Err(JsonDecodeError::empty_input(
                JsonDecodeStage::Normalize,
                raw_input_bytes,
                Some(input.len()),
                self.policy.diagnostic_policy(),
            ))
        } else {
            Ok(input)
        }
    }

    /// Rejects input empty under the active whitespace policy.
    ///
    /// # Type Parameters
    ///
    /// * `'input` - Lifetime of the returned slice borrowed from `input`.
    /// * `R` - Resource kind in the resulting decode error.
    /// * `Q` - Quantity type used by the decode error.
    ///
    /// # Parameters
    ///
    /// * `input` - Text checked for emptiness.
    /// * `raw_input_bytes` - Original input size retained in any error.
    ///
    /// # Returns
    ///
    /// The original borrowed slice when it is non-empty under the active
    /// whitespace policy.
    ///
    /// # Errors
    ///
    /// Returns an empty-input error when the active policy considers the text
    /// empty.
    fn require_non_empty<'input, R, Q>(
        &self,
        input: &'input str,
        raw_input_bytes: usize,
    ) -> Result<&'input str, JsonDecodeError<R, Q>>
    where
        Q: ResourceQuantity,
    {
        let empty = if self.policy.trim_whitespace() {
            input.trim().is_empty()
        } else {
            input.is_empty()
        };
        if empty {
            Err(JsonDecodeError::empty_input(
                JsonDecodeStage::Normalize,
                raw_input_bytes,
                None,
                self.policy.diagnostic_policy(),
            ))
        } else {
            Ok(input)
        }
    }

    /// Scans the post-slice text before allocating escaped output.
    ///
    /// # Parameters
    ///
    /// * `input` - Text remaining after whitespace, BOM, and fence handling.
    ///
    /// # Returns
    ///
    /// The normalized byte length and whether escaping requires owned output.
    #[inline]
    #[must_use]
    fn scan_normalized_size(&self, input: &str) -> (usize, bool) {
        ControlCharacterEscaper::scan(input, self.policy.escape_control_chars_in_strings())
    }

    /// Charges raw input bytes and retains the complete measured failure.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Resource kind tracked by the decoding attempt.
    /// * `Q` - Quantity type used for resource accounting.
    ///
    /// # Parameters
    ///
    /// * `attempt` - Attempt receiving the raw input charge.
    /// * `amount` - Number of raw input bytes to charge.
    ///
    /// # Returns
    ///
    /// `()` after the attempt accepts the raw byte charge.
    ///
    /// # Errors
    ///
    /// Returns the budget failure with the measured raw byte amount when the
    /// attempt cannot accept the charge.
    fn consume_raw_input<R, Q>(
        &self,
        attempt: &mut JsonDecodeAttempt<'_, R, Q>,
        amount: usize,
    ) -> Result<(), JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        attempt.try_consume_input_bytes(amount).map_err(|source| {
            JsonDecodeError::budget(
                source,
                JsonDecodeStage::Input,
                amount,
                None,
                self.policy.diagnostic_policy(),
            )
        })
    }

    /// Charges normalized bytes before allocating escaped output.
    ///
    /// # Type Parameters
    ///
    /// * `R` - Resource kind tracked by the decoding attempt.
    /// * `Q` - Quantity type used for resource accounting.
    ///
    /// # Parameters
    ///
    /// * `attempt` - Attempt receiving the normalized input charge.
    /// * `raw_input_bytes` - Original input size retained in any error.
    /// * `normalized_input_bytes` - Normalized size to charge before
    ///   allocation.
    ///
    /// # Returns
    ///
    /// `()` after the attempt accepts the normalized byte charge.
    ///
    /// # Errors
    ///
    /// Returns the budget failure with raw and normalized byte measurements
    /// when the attempt cannot accept the normalized charge.
    fn consume_normalized_input<R, Q>(
        &self,
        attempt: &mut JsonDecodeAttempt<'_, R, Q>,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Result<(), JsonDecodeError<R, Q>>
    where
        R: Clone,
        Q: ResourceQuantity,
    {
        attempt
            .try_consume_normalized_input_bytes(normalized_input_bytes)
            .map_err(|source| {
                JsonDecodeError::budget(
                    source,
                    JsonDecodeStage::Normalize,
                    raw_input_bytes,
                    Some(normalized_input_bytes),
                    self.policy.diagnostic_policy(),
                )
            })
    }

    /// Trims a borrowed input slice when trimming is enabled.
    ///
    /// # Parameters
    ///
    /// * `input` - Text to trim according to the configured policy.
    ///
    /// # Returns
    ///
    /// A borrowed slice with surrounding whitespace removed when trimming is
    /// enabled, or the original slice otherwise.
    #[inline]
    #[must_use]
    fn trim_if_enabled<'input>(&self, input: &'input str) -> &'input str {
        if self.policy.trim_whitespace() {
            input.trim()
        } else {
            input
        }
    }

    /// Removes one leading UTF-8 byte order mark when configured.
    ///
    /// # Parameters
    ///
    /// * `input` - Text whose leading byte order mark is checked.
    ///
    /// # Returns
    ///
    /// A borrowed slice without one leading BOM when configured, or the
    /// original slice otherwise.
    #[inline]
    #[must_use]
    fn strip_utf8_bom<'input>(&self, input: &'input str) -> &'input str {
        if self.policy.strip_utf8_bom() {
            input.strip_prefix('\u{feff}').unwrap_or(input)
        } else {
            input
        }
    }
}
