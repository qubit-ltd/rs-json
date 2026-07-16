// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
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
    fn default() -> Self {
        Self::new(JsonDecodeOptions::default())
    }
}

impl LenientJsonNormalizer {
    /// Creates a normalizer with the provided lenient decoding options.
    #[must_use]
    pub(crate) const fn new(options: JsonDecodeOptions) -> Self {
        Self { options }
    }

    /// Returns the configuration used by this normalizer.
    #[must_use]
    pub(crate) const fn options(&self) -> &JsonDecodeOptions {
        &self.options
    }

    /// Normalizes one raw JSON text input into text ready for parsing.
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
    #[inline]
    fn trim_if_enabled<'a>(&self, input: &'a str) -> &'a str {
        if self.options.trim_whitespace() {
            input.trim()
        } else {
            input
        }
    }

    /// Trims normalized text while preserving its ownership mode where
    /// possible.
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
    #[inline]
    fn strip_utf8_bom<'a>(&self, input: &'a str) -> &'a str {
        if self.options.strip_utf8_bom() {
            input.strip_prefix('\u{feff}').unwrap_or(input)
        } else {
            input
        }
    }

    /// Removes one supported outer Markdown code fence when configured.
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
    fn is_json_code_fence_tag(tag: &str) -> bool {
        let language = tag.split_whitespace().next().unwrap_or("");
        language.is_empty()
            || language.eq_ignore_ascii_case("json")
            || language.eq_ignore_ascii_case("jsonc")
    }

    /// Removes a compatible closing fence from content when present.
    fn strip_markdown_closing_fence(
        content: &str,
        opening_fence: MarkdownFence,
    ) -> Option<&str> {
        let trimmed_end = content.trim_end_matches(char::is_whitespace);
        let closing_line_start = trimmed_end
            .rfind('\n')
            .max(trimmed_end.rfind('\r'))
            .map_or(0, |index| index + 1);
        let closing_line = trimmed_end[closing_line_start..].trim();
        let closing_len =
            Self::same_marker_fence_len(closing_line, opening_fence.marker)?;
        if closing_len == closing_line.len()
            && closing_len >= opening_fence.marker_len
        {
            Some(&content[..closing_line_start])
        } else {
            None
        }
    }

    /// Returns the marker run length when a line starts with the same marker.
    fn same_marker_fence_len(line: &str, marker: u8) -> Option<usize> {
        let count = line.bytes().take_while(|byte| *byte == marker).count();
        (count >= 3).then_some(count)
    }
}
