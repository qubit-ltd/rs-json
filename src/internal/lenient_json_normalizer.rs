// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private normalization pipeline for lenient JSON decoding.

use std::borrow::Cow;

use crate::{
    JsonDecodeError,
    JsonDecodeOptions,
    MarkdownFenceClosing,
    MarkdownFencePolicy,
};

use super::{
    control_character_escaper::ControlCharacterEscaper,
    markdown_fence::MarkdownFence,
};

/// Normalizes one raw JSON text input before JSON parsing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LenientJsonNormalizer {
    /// Stores the option set used by the normalizer.
    options: JsonDecodeOptions,
}

impl Default for LenientJsonNormalizer {
    /// Creates a normalizer using the default lenient option set.
    ///
    /// # Returns
    ///
    /// A normalizer configured with [`JsonDecodeOptions::default`].
    #[inline(always)]
    fn default() -> Self {
        Self::new(JsonDecodeOptions::default())
    }
}

impl LenientJsonNormalizer {
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
    pub(crate) const fn new(options: JsonDecodeOptions) -> Self {
        Self { options }
    }

    /// Returns the configuration used by this normalizer.
    ///
    /// # Returns
    ///
    /// The immutable normalization options.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn options(&self) -> &JsonDecodeOptions {
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
    /// Borrowed input when no rewrite is needed, or owned normalized text when
    /// control characters require escaping.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when the raw input exceeds its configured
    /// limit or becomes empty at a normalization boundary.
    pub(crate) fn normalize<'a>(
        &self,
        input: &'a str,
    ) -> Result<Cow<'a, str>, JsonDecodeError> {
        let raw_input_bytes = input.len();
        self.require_within_size_limit(input)?;
        let input = self.require_non_empty(input, raw_input_bytes)?;
        let input = self.trim_if_enabled(input);
        let input = self.strip_utf8_bom(input);
        let input = self.trim_if_enabled(input);
        let input = self.strip_markdown_code_fence(input);
        let input = self.trim_if_enabled(input);
        let input = ControlCharacterEscaper::escape(
            input,
            self.options.escape_control_chars_in_strings(),
        );
        let input = self.trim_cow_if_enabled(input);

        if input.is_empty() {
            Err(JsonDecodeError::empty_input(
                raw_input_bytes,
                Some(input.len()),
                self.options.error_privacy_policy(),
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
    /// Returns [`JsonDecodeError`] when the input is empty under the active
    /// whitespace policy.
    fn require_non_empty<'a>(
        &self,
        input: &'a str,
        raw_input_bytes: usize,
    ) -> Result<&'a str, JsonDecodeError> {
        if self.options.trim_whitespace() {
            if input.trim().is_empty() {
                return Err(JsonDecodeError::empty_input(
                    raw_input_bytes,
                    None,
                    self.options.error_privacy_policy(),
                ));
            }
        } else if input.is_empty() {
            return Err(JsonDecodeError::empty_input(
                raw_input_bytes,
                None,
                self.options.error_privacy_policy(),
            ));
        }
        Ok(input)
    }

    /// Rejects raw input that exceeds the configured size limit.
    ///
    /// # Parameters
    ///
    /// * `input` - Raw input whose byte length is checked.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the input is within the limit or no limit is configured.
    ///
    /// # Errors
    ///
    /// Returns [`JsonDecodeError`] when the input is larger than the configured
    /// raw byte limit.
    fn require_within_size_limit(
        &self,
        input: &str,
    ) -> Result<(), JsonDecodeError> {
        if let Some(limit) = self.options.max_input_bytes() {
            let size = input.len();
            if size > limit {
                return Err(JsonDecodeError::input_too_large(
                    size,
                    limit,
                    self.options.error_privacy_policy(),
                ));
            }
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

    /// Trims normalized text while preserving its ownership mode where
    /// possible.
    ///
    /// # Parameters
    ///
    /// * `input` - Borrowed or owned normalized text.
    ///
    /// # Returns
    ///
    /// Conditionally trimmed text, preserving borrowed storage and retaining
    /// owned storage when no trim is needed.
    #[must_use]
    fn trim_cow_if_enabled<'a>(&self, input: Cow<'a, str>) -> Cow<'a, str> {
        if !self.options.trim_whitespace() {
            return input;
        }
        match input {
            Cow::Borrowed(text) => Cow::Borrowed(text.trim()),
            Cow::Owned(text) => {
                let trimmed = text.trim();
                if trimmed.len() == text.len() {
                    Cow::Owned(text)
                } else {
                    Cow::Owned(trimmed.to_string())
                }
            }
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

    /// Removes one supported outer Markdown code fence when configured.
    ///
    /// # Parameters
    ///
    /// * `input` - Text to inspect for one outer Markdown code fence.
    ///
    /// # Returns
    ///
    /// The fenced body when the active policy accepts the fence, or the
    /// unchanged input otherwise.
    #[must_use]
    fn strip_markdown_code_fence<'a>(&self, input: &'a str) -> &'a str {
        let (json_only, closing) = match self.options.markdown_fence_policy() {
            MarkdownFencePolicy::Disabled => return input,
            MarkdownFencePolicy::Any { closing } => (false, closing),
            MarkdownFencePolicy::JsonOnly { closing } => (true, closing),
        };
        let Some(opening_fence) = Self::opening_markdown_fence(input) else {
            return input;
        };
        let Some((line_end, content_start)) = Self::first_line_break(input)
        else {
            return input;
        };
        let opening_tag = input[opening_fence.marker_end..line_end].trim();
        if json_only && !Self::is_json_code_fence_tag(opening_tag) {
            return input;
        }
        let content = &input[content_start..];
        if let Some(without_close) =
            Self::strip_markdown_closing_fence(content, opening_fence)
        {
            return without_close;
        }
        if closing == MarkdownFenceClosing::Required {
            input
        } else {
            content
        }
    }

    /// Returns a recognized opening Markdown fence when present.
    ///
    /// # Parameters
    ///
    /// * `input` - Text beginning at a possible fence opening.
    ///
    /// # Returns
    ///
    /// `Some(fence)` when `input` begins with a supported marker after at most
    /// three spaces, or `None` otherwise.
    fn opening_markdown_fence(input: &str) -> Option<MarkdownFence> {
        let indent_len = input.bytes().take_while(|byte| *byte == b' ').count();
        if indent_len > 3 {
            return None;
        }
        let marker = *input.as_bytes().get(indent_len)?;
        if marker != b'`' && marker != b'~' {
            return None;
        }
        let marker_len = input[indent_len..]
            .bytes()
            .take_while(|byte| *byte == marker)
            .count();
        (marker_len >= 3).then_some(MarkdownFence {
            marker,
            marker_len,
            marker_end: indent_len + marker_len,
        })
    }

    /// Returns the end of the first line and the start of the next line.
    ///
    /// # Parameters
    ///
    /// * `input` - Text whose first line break is located.
    ///
    /// # Returns
    ///
    /// `Some((line_end, next_line_start))` for LF, CRLF, or CR input, or
    /// `None` when no line break exists.
    fn first_line_break(input: &str) -> Option<(usize, usize)> {
        let newline = input.find('\n');
        let carriage_return = input.find('\r');
        match (newline, carriage_return) {
            (Some(newline), Some(carriage_return))
                if carriage_return < newline =>
            {
                let content_start = if newline == carriage_return + 1 {
                    newline + 1
                } else {
                    carriage_return + 1
                };
                Some((carriage_return, content_start))
            }
            (Some(newline), _) => Some((newline, newline + 1)),
            (None, Some(carriage_return)) => {
                Some((carriage_return, carriage_return + 1))
            }
            (None, None) => None,
        }
    }

    /// Returns whether a fenced info string should be treated as JSON.
    ///
    /// # Parameters
    ///
    /// * `tag` - Markdown fence info string.
    ///
    /// # Returns
    ///
    /// `true` for an empty, `json`, or `jsonc` first token, ignoring ASCII
    /// case; otherwise, `false`. The `jsonc` token is accepted only as a fence
    /// label and does not enable non-standard JSON grammar.
    #[inline]
    #[must_use]
    fn is_json_code_fence_tag(tag: &str) -> bool {
        let language = tag.split_whitespace().next().unwrap_or("");
        language.is_empty()
            || language.eq_ignore_ascii_case("json")
            || language.eq_ignore_ascii_case("jsonc")
    }

    /// Removes a compatible closing fence from content when present.
    ///
    /// # Parameters
    ///
    /// * `content` - Fenced body and possible closing fence.
    /// * `opening_fence` - Opening marker that the closing line must match.
    ///
    /// # Returns
    ///
    /// `Some(body)` when the final non-whitespace line is a compatible closing
    /// fence, or `None` otherwise.
    fn strip_markdown_closing_fence(
        content: &str,
        opening_fence: MarkdownFence,
    ) -> Option<&str> {
        let trimmed_end = content.trim_end_matches([' ', '\t', '\n', '\r']);
        let closing_line_start = trimmed_end
            .rfind('\n')
            .max(trimmed_end.rfind('\r'))
            .map_or(0, |index| index + 1);
        let closing_line = &trimmed_end[closing_line_start..];
        let indent_len = closing_line
            .bytes()
            .take_while(|byte| *byte == b' ')
            .count();
        if indent_len > 3 {
            return None;
        }
        let marker_line = &closing_line[indent_len..];
        let closing_len =
            Self::same_marker_fence_len(marker_line, opening_fence.marker)?;
        if closing_len == marker_line.len()
            && closing_len >= opening_fence.marker_len
        {
            Some(&content[..closing_line_start])
        } else {
            None
        }
    }

    /// Returns the marker run length when a line starts with the same marker.
    ///
    /// # Parameters
    ///
    /// * `line` - Candidate closing-fence line.
    /// * `marker` - Opening fence marker byte.
    ///
    /// # Returns
    ///
    /// `Some(length)` for a run of at least three matching marker bytes, or
    /// `None` otherwise.
    #[inline]
    fn same_marker_fence_len(line: &str, marker: u8) -> Option<usize> {
        let count = line.bytes().take_while(|byte| *byte == marker).count();
        (count >= 3).then_some(count)
    }
}
