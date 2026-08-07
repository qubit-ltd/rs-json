// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the [`JsonDecodeError`] type used by the public decoder API.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use super::internal::JsonInputSizeLimit;
use crate::ErrorPrivacyPolicy;
use crate::JsonDecodeErrorKind;
use crate::JsonDecodeStage;
use crate::JsonTopLevelKind;

/// Error returned when lenient JSON decoding fails.
///
/// This type exposes immutable diagnostics so its error category, stage, and
/// associated input metadata always remain consistent with one another.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct JsonDecodeError {
    /// Stores the stable category of the decoding failure.
    kind: JsonDecodeErrorKind,
    /// Stores the pipeline stage that produced the failure.
    stage: JsonDecodeStage,
    /// Stores the privacy policy applied while constructing diagnostics.
    privacy_policy: ErrorPrivacyPolicy,
    /// Stores the human-readable diagnostic message.
    message: String,
    /// Stores the expected constrained top-level kind, when applicable.
    expected_top_level: Option<JsonTopLevelKind>,
    /// Stores the actual constrained top-level kind, when applicable.
    actual_top_level: Option<JsonTopLevelKind>,
    /// Stores the input byte length before normalization.
    raw_input_bytes: usize,
    /// Stores the normalized byte length when normalization completed.
    normalized_input_bytes: Option<usize>,
    /// Stores the configured size limit that rejected the input.
    size_limit: Option<JsonInputSizeLimit>,
    /// Stores the one-based parser line in normalized text when available.
    normalized_line: Option<usize>,
    /// Stores the one-based parser column in normalized text when available.
    normalized_column: Option<usize>,
    /// Stores internal error details exposed only when diagnostics permit it.
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl JsonDecodeError {
    /// Creates an error for raw bytes that are not valid UTF-8.
    ///
    /// # Parameters
    ///
    /// * `error` - UTF-8 validation error.
    /// * `raw_input_bytes` - Raw input length in bytes.
    /// * `privacy_policy` - Policy controlling retained diagnostics.
    ///
    /// # Returns
    ///
    /// An invalid-UTF-8 error that exposes the source only in detailed mode.
    #[must_use]
    pub(crate) fn invalid_utf8(
        error: std::str::Utf8Error,
        raw_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        let utf8_error = Arc::new(error);
        let message = match privacy_policy {
            ErrorPrivacyPolicy::Redacted => {
                "Failed to decode JSON input as UTF-8".to_string()
            }
            ErrorPrivacyPolicy::Detailed => {
                format!("Failed to decode JSON input as UTF-8: {utf8_error}")
            }
        };
        let source = Some(utf8_error as Arc<dyn Error + Send + Sync>);
        Self {
            kind: JsonDecodeErrorKind::InvalidUtf8,
            stage: JsonDecodeStage::DecodeText,
            privacy_policy,
            message,
            expected_top_level: None,
            actual_top_level: None,
            raw_input_bytes,
            normalized_input_bytes: None,
            size_limit: None,
            normalized_line: None,
            normalized_column: None,
            source,
        }
    }

    /// Creates an error for raw input that exceeds the configured size limit.
    ///
    /// # Parameters
    ///
    /// * `raw_input_bytes` - Raw input length in bytes.
    /// * `max_input_bytes` - Configured maximum raw input length.
    /// * `privacy_policy` - Privacy policy active during normalization.
    ///
    /// # Returns
    ///
    /// An input-too-large error containing the supplied size diagnostics.
    #[must_use]
    pub(crate) fn input_too_large(
        raw_input_bytes: usize,
        max_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            kind: JsonDecodeErrorKind::InputTooLarge,
            stage: JsonDecodeStage::Normalize,
            privacy_policy,
            message: format!(
                "JSON input is too large: {raw_input_bytes} bytes exceed configured limit {max_input_bytes} bytes"
            ),
            expected_top_level: None,
            actual_top_level: None,
            raw_input_bytes,
            normalized_input_bytes: None,
            size_limit: Some(JsonInputSizeLimit::Raw(max_input_bytes)),
            normalized_line: None,
            normalized_column: None,
            source: None,
        }
    }

    /// Creates an error for normalized input that exceeds its configured limit.
    ///
    /// # Parameters
    ///
    /// * `raw_input_bytes` - Raw input length in bytes.
    /// * `normalized_input_bytes` - Calculated normalized input length in
    ///   bytes.
    /// * `max_normalized_bytes` - Configured maximum normalized input length.
    /// * `privacy_policy` - Privacy policy active during normalization.
    ///
    /// # Returns
    ///
    /// An input-too-large error containing the supplied normalized size
    /// diagnostics.
    #[must_use]
    pub(crate) fn normalized_input_too_large(
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        max_normalized_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            kind: JsonDecodeErrorKind::InputTooLarge,
            stage: JsonDecodeStage::Normalize,
            privacy_policy,
            message: format!(
                "Normalized JSON input is too large: {normalized_input_bytes} bytes exceed configured limit {max_normalized_bytes} bytes"
            ),
            expected_top_level: None,
            actual_top_level: None,
            raw_input_bytes,
            normalized_input_bytes: Some(normalized_input_bytes),
            size_limit: Some(JsonInputSizeLimit::Normalized(
                max_normalized_bytes,
            )),
            normalized_line: None,
            normalized_column: None,
            source: None,
        }
    }

    /// Creates an error for input that is empty at a normalization boundary.
    ///
    /// # Parameters
    ///
    /// * `raw_input_bytes` - Raw input length in bytes.
    /// * `normalized_input_bytes` - Normalized length when normalization
    ///   completed, or `None` when the input was rejected earlier.
    /// * `privacy_policy` - Privacy policy active during normalization.
    ///
    /// # Returns
    ///
    /// An empty-input error containing the available size diagnostics.
    #[must_use]
    pub(crate) fn empty_input(
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            kind: JsonDecodeErrorKind::EmptyInput,
            stage: JsonDecodeStage::Normalize,
            privacy_policy,
            message: "JSON input is empty after normalization".to_string(),
            expected_top_level: None,
            actual_top_level: None,
            raw_input_bytes,
            normalized_input_bytes,
            size_limit: None,
            normalized_line: None,
            normalized_column: None,
            source: None,
        }
    }

    /// Creates an error for invalid normalized JSON syntax.
    ///
    /// # Parameters
    ///
    /// * `error` - Serde parser error.
    /// * `raw_input_bytes` - Raw input length in bytes.
    /// * `normalized_input_bytes` - Normalized input length in bytes.
    /// * `privacy_policy` - Policy controlling retained serde diagnostics.
    ///
    /// # Returns
    ///
    /// An invalid-JSON error with stable location and size metadata.
    #[inline(always)]
    #[must_use]
    pub(crate) fn invalid_json(
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self::from_serde_error(
            JsonDecodeErrorKind::InvalidJson,
            JsonDecodeStage::Parse,
            "Failed to parse JSON",
            error,
            raw_input_bytes,
            normalized_input_bytes,
            privacy_policy,
        )
    }

    /// Creates an error for a valid JSON value with an unexpected top-level
    /// kind.
    ///
    /// # Parameters
    ///
    /// * `expected` - Required top-level kind.
    /// * `actual` - Observed top-level kind.
    /// * `raw_input_bytes` - Raw input length in bytes.
    /// * `normalized_input_bytes` - Normalized input length in bytes.
    /// * `privacy_policy` - Privacy policy active during decoding.
    ///
    /// # Returns
    ///
    /// A top-level-check error containing the expected and actual kinds.
    #[must_use]
    pub(crate) fn unexpected_top_level(
        expected: JsonTopLevelKind,
        actual: JsonTopLevelKind,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            kind: JsonDecodeErrorKind::UnexpectedTopLevel,
            stage: JsonDecodeStage::TopLevelCheck,
            privacy_policy,
            message: format!(
                "Unexpected JSON top-level type: expected {expected}, got {actual}"
            ),
            expected_top_level: Some(expected),
            actual_top_level: Some(actual),
            raw_input_bytes,
            normalized_input_bytes: Some(normalized_input_bytes),
            size_limit: None,
            normalized_line: None,
            normalized_column: None,
            source: None,
        }
    }

    /// Creates an error for valid JSON that cannot deserialize into the target.
    ///
    /// # Parameters
    ///
    /// * `error` - Serde deserialization error.
    /// * `raw_input_bytes` - Raw input length in bytes.
    /// * `normalized_input_bytes` - Normalized input length in bytes.
    /// * `privacy_policy` - Policy controlling retained serde diagnostics.
    ///
    /// # Returns
    ///
    /// A deserialization error with stable location and size metadata.
    #[inline(always)]
    #[must_use]
    pub(crate) fn deserialize(
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self::from_serde_error(
            JsonDecodeErrorKind::Deserialize,
            JsonDecodeStage::Deserialize,
            "Failed to deserialize JSON value",
            error,
            raw_input_bytes,
            normalized_input_bytes,
            privacy_policy,
        )
    }

    /// Creates a decoder error from a serde error and privacy policy.
    ///
    /// # Parameters
    ///
    /// * `kind` - Stable public error category.
    /// * `stage` - Decoder stage that produced the error.
    /// * `prefix` - Stable message prefix.
    /// * `error` - Serde error carrying location and optional details.
    /// * `raw_input_bytes` - Raw input length in bytes.
    /// * `normalized_input_bytes` - Normalized input length in bytes.
    /// * `privacy_policy` - Policy controlling retained serde diagnostics.
    ///
    /// # Returns
    ///
    /// A decoder error that always retains stable metadata and retains the
    /// serde source only under the detailed privacy policy.
    #[must_use]
    fn from_serde_error(
        kind: JsonDecodeErrorKind,
        stage: JsonDecodeStage,
        prefix: &str,
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        let line = error.line();
        let column = error.column();
        let (message, source) = match privacy_policy {
            ErrorPrivacyPolicy::Redacted => {
                (Self::redacted_message(prefix, line, column), None)
            }
            ErrorPrivacyPolicy::Detailed => (
                format!("{prefix}: {error}"),
                Some(Arc::new(error) as Arc<dyn Error + Send + Sync>),
            ),
        };
        Self {
            kind,
            stage,
            privacy_policy,
            message,
            expected_top_level: None,
            actual_top_level: None,
            raw_input_bytes,
            normalized_input_bytes: Some(normalized_input_bytes),
            size_limit: None,
            normalized_line: (line > 0).then_some(line),
            normalized_column: (column > 0).then_some(column),
            source,
        }
    }

    /// Returns the stable category of this decoding failure.
    ///
    /// # Returns
    ///
    /// The stable error category.
    #[inline(always)]
    pub const fn kind(&self) -> JsonDecodeErrorKind {
        self.kind
    }

    /// Returns the decoding stage that produced this error.
    ///
    /// # Returns
    ///
    /// The decoder stage where the failure occurred.
    #[inline(always)]
    pub const fn stage(&self) -> JsonDecodeStage {
        self.stage
    }

    /// Returns the privacy policy applied when this error was constructed.
    ///
    /// # Returns
    ///
    /// The effective error privacy policy.
    #[inline(always)]
    pub const fn privacy_policy(&self) -> ErrorPrivacyPolicy {
        self.privacy_policy
    }

    /// Returns the human-readable diagnostic message.
    ///
    /// # Returns
    ///
    /// The stable redacted message or explicitly requested detailed message.
    #[inline(always)]
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the required top-level JSON kind for a constrained decode.
    ///
    /// # Returns
    ///
    /// `Some(kind)` when constrained decoding rejected a valid top-level
    /// value; otherwise, `None`.
    #[inline(always)]
    pub const fn expected_top_level(&self) -> Option<JsonTopLevelKind> {
        self.expected_top_level
    }

    /// Returns the observed top-level JSON kind for a constrained decode.
    ///
    /// # Returns
    ///
    /// `Some(kind)` when constrained decoding rejected a valid top-level
    /// value; otherwise, `None`.
    #[inline(always)]
    pub const fn actual_top_level(&self) -> Option<JsonTopLevelKind> {
        self.actual_top_level
    }

    /// Returns the byte length of the input before normalization.
    ///
    /// # Returns
    ///
    /// The raw input length in bytes.
    #[inline(always)]
    #[must_use]
    pub const fn raw_input_bytes(&self) -> usize {
        self.raw_input_bytes
    }

    /// Returns the valid UTF-8 prefix length for invalid byte input.
    ///
    /// # Returns
    ///
    /// `Some(length)` for [`JsonDecodeErrorKind::InvalidUtf8`], including zero
    /// when the first byte is invalid; otherwise, `None`.
    #[inline(always)]
    pub fn utf8_valid_up_to(&self) -> Option<usize> {
        self.source
            .as_deref()
            .and_then(|error| error.downcast_ref::<std::str::Utf8Error>())
            .map(std::str::Utf8Error::valid_up_to)
    }

    /// Returns the known length of the invalid UTF-8 sequence.
    ///
    /// # Returns
    ///
    /// `Some(length)` when the invalid sequence length is known, or `None` for
    /// an incomplete trailing sequence and for non-UTF-8 errors.
    #[inline(always)]
    pub fn utf8_error_len(&self) -> Option<usize> {
        self.source
            .as_deref()
            .and_then(|error| error.downcast_ref::<std::str::Utf8Error>())
            .and_then(std::str::Utf8Error::error_len)
    }

    /// Returns the byte length of normalized JSON text.
    ///
    /// # Returns
    ///
    /// `Some(length)` when normalization completed before the failure, or
    /// `None` when the input was rejected before a normalized length existed.
    #[inline(always)]
    pub const fn normalized_input_bytes(&self) -> Option<usize> {
        self.normalized_input_bytes
    }

    /// Returns the configured raw-input limit for a size failure.
    ///
    /// # Returns
    ///
    /// `Some(limit)` when raw input exceeded its configured limit, or `None`
    /// for normalized-size and non-size failures.
    #[inline(always)]
    pub const fn max_input_bytes(&self) -> Option<usize> {
        match self.size_limit {
            Some(JsonInputSizeLimit::Raw(limit)) => Some(limit),
            Some(JsonInputSizeLimit::Normalized(_)) | None => None,
        }
    }

    /// Returns the configured normalized-input limit for a size failure.
    ///
    /// # Returns
    ///
    /// `Some(limit)` when normalized JSON exceeded its configured limit, or
    /// `None` for raw-size and non-size failures.
    #[inline(always)]
    pub const fn max_normalized_bytes(&self) -> Option<usize> {
        match self.size_limit {
            Some(JsonInputSizeLimit::Normalized(limit)) => Some(limit),
            Some(JsonInputSizeLimit::Raw(_)) | None => None,
        }
    }

    /// Returns the parser line in normalized JSON text.
    ///
    /// # Returns
    ///
    /// `Some(line)` with a one-based line number when serde reported one, or
    /// `None` when no parser location is available.
    #[inline(always)]
    pub const fn normalized_line(&self) -> Option<usize> {
        self.normalized_line
    }

    /// Returns the parser column in normalized JSON text.
    ///
    /// # Returns
    ///
    /// `Some(column)` with a one-based column number when serde reported one,
    /// or `None` when no parser location is available.
    #[inline(always)]
    pub const fn normalized_column(&self) -> Option<usize> {
        self.normalized_column
    }

    /// Builds a diagnostic that contains only stable text and parser location.
    ///
    /// # Parameters
    ///
    /// * `prefix` - Stable error-message prefix.
    /// * `line` - One-based parser line, or zero when unavailable.
    /// * `column` - One-based parser column, or zero when unavailable.
    ///
    /// # Returns
    ///
    /// A message containing the prefix and each available normalized location.
    #[must_use]
    fn redacted_message(prefix: &str, line: usize, column: usize) -> String {
        match (line > 0, column > 0) {
            (true, true) => {
                format!("{prefix} at normalized line {line} column {column}")
            }
            (true, false) => {
                format!("{prefix} at normalized line {line}")
            }
            (false, true) => {
                format!("{prefix} at normalized column {column}")
            }
            (false, false) => prefix.to_string(),
        }
    }
}

impl PartialEq for JsonDecodeError {
    /// Compares all stable error fields while ignoring the retained source.
    ///
    /// # Parameters
    ///
    /// * `other` - Error to compare with this error.
    ///
    /// # Returns
    ///
    /// `true` when every stable diagnostic field is equal; otherwise, `false`.
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.stage == other.stage
            && self.privacy_policy == other.privacy_policy
            && self.message == other.message
            && self.expected_top_level == other.expected_top_level
            && self.actual_top_level == other.actual_top_level
            && self.raw_input_bytes == other.raw_input_bytes
            && self.utf8_valid_up_to() == other.utf8_valid_up_to()
            && self.utf8_error_len() == other.utf8_error_len()
            && self.normalized_input_bytes == other.normalized_input_bytes
            && self.size_limit == other.size_limit
            && self.normalized_line == other.normalized_line
            && self.normalized_column == other.normalized_column
    }
}

impl Eq for JsonDecodeError {}

impl fmt::Display for JsonDecodeError {
    /// Writes the configured human-readable diagnostic message.
    ///
    /// # Parameters
    ///
    /// * `f` - Destination formatter.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the message is written successfully.
    ///
    /// # Errors
    ///
    /// Returns a formatting error when the destination formatter rejects the
    /// write.
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for JsonDecodeError {
    /// Returns the decoder source under detailed privacy mode.
    ///
    /// # Returns
    ///
    /// `Some(source)` when detailed diagnostics expose a UTF-8 or serde
    /// error, or `None` for redacted and source-free failures.
    #[inline(always)]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.privacy_policy {
            ErrorPrivacyPolicy::Redacted => None,
            ErrorPrivacyPolicy::Detailed => self
                .source
                .as_deref()
                .map(|error| error as &(dyn Error + 'static)),
        }
    }
}
