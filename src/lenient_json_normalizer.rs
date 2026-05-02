/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Internal normalization utilities used by the lenient JSON decoder.
//!

use std::borrow::Cow;

use crate::{JsonDecodeError, JsonDecodeOptions};

/// Normalizes one raw JSON text input before JSON parsing.
///
/// The object holds normalization options and applies all supported preprocessing
/// rules in the same order for every `normalize` call.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LenientJsonNormalizer {
    /// Stores the option set used by the normalizer.
    options: JsonDecodeOptions,
}

/// Describes one recognized Markdown code fence opening line.
#[derive(Debug, Clone, Copy)]
struct MarkdownFence {
    /// Stores the byte marker used by the fence.
    marker: u8,
    /// Stores the number of repeated marker bytes in the opening fence.
    marker_len: usize,
    /// Stores the byte index immediately after the opening marker run.
    marker_end: usize,
}

impl Default for LenientJsonNormalizer {
    fn default() -> Self {
        Self::new(JsonDecodeOptions::default())
    }
}

impl LenientJsonNormalizer {
    /// Creates a normalizer with the provided lenient decoding options.
    ///
    /// The options are copied into the object so each `normalize` call uses a
    /// consistent policy without external mutation.
    #[must_use]
    pub(crate) const fn new(options: JsonDecodeOptions) -> Self {
        Self { options }
    }

    /// Returns the configuration used by this normalizer.
    #[must_use]
    pub(crate) const fn options(&self) -> &JsonDecodeOptions {
        &self.options
    }

    /// Normalizes one raw JSON text input and returns a parsed-ready text slice.
    ///
    /// The pipeline is intentionally narrow: it trims whitespace, strips an
    /// optional BOM, optionally removes a Markdown code fence, escapes control
    /// characters in strings, and finally validates non-emptiness again.
    pub(crate) fn normalize<'a>(&self, input: &'a str) -> Result<Cow<'a, str>, JsonDecodeError> {
        self.require_within_size_limit(input)?;
        let input = self.require_non_empty(input)?;
        let input = self.trim_if_enabled(input);
        let input = self.strip_utf8_bom(input);
        let input = self.trim_if_enabled(input);
        let input = self.strip_markdown_code_fence(input);
        let input = self.trim_if_enabled(input);
        let input = self.escape_control_chars_in_json_strings(input);
        let input = self.trim_cow_if_enabled(input);

        if input.is_empty() {
            Err(JsonDecodeError::empty_input())
        } else {
            Ok(input)
        }
    }

    /// Verifies that the input is not empty under the configured policy.
    ///
    /// If `trim_whitespace` is enabled, whitespace-only input is rejected as
    /// empty; otherwise only zero-length input is rejected.
    fn require_non_empty<'a>(&self, input: &'a str) -> Result<&'a str, JsonDecodeError> {
        if self.options.trim_whitespace {
            if input.trim().is_empty() {
                Err(JsonDecodeError::empty_input())
            } else {
                Ok(input)
            }
        } else if input.is_empty() {
            Err(JsonDecodeError::empty_input())
        } else {
            Ok(input)
        }
    }

    /// Verifies that the raw input length does not exceed the configured
    /// maximum, when one is configured.
    fn require_within_size_limit(&self, input: &str) -> Result<(), JsonDecodeError> {
        if let Some(limit) = self.options.max_input_bytes {
            let size = input.len();
            if size > limit {
                return Err(JsonDecodeError::input_too_large(size, limit));
            }
        }
        Ok(())
    }

    /// Trims a borrowed input slice if trimming is enabled.
    ///
    /// This helper borrows and never allocates when trimming is disabled.
    fn trim_if_enabled<'a>(&self, input: &'a str) -> &'a str {
        if self.options.trim_whitespace {
            input.trim()
        } else {
            input
        }
    }

    /// Trims the normalized text when trimming is enabled.
    ///
    /// Borrowed values remain borrowed, and owned values are copied only when
    /// trimming removes characters.
    fn trim_cow_if_enabled<'a>(&self, input: Cow<'a, str>) -> Cow<'a, str> {
        if !self.options.trim_whitespace {
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

    /// Removes an optional UTF-8 BOM before parsing.
    ///
    /// If no BOM exists, the input is returned unchanged.
    fn strip_utf8_bom<'a>(&self, input: &'a str) -> &'a str {
        if self.options.strip_utf8_bom {
            input.strip_prefix('\u{feff}').unwrap_or(input)
        } else {
            input
        }
    }

    /// Removes one outer Markdown code fence when enabled.
    ///
    /// The helper only strips a backtick fence that starts at the beginning of
    /// input and uses at least three backticks or tildes. Up to three leading
    /// spaces before the opening marker are accepted. If a valid closing fence
    /// is present after trimming the trailing side, it is also removed.
    fn strip_markdown_code_fence<'a>(&self, input: &'a str) -> &'a str {
        if !self.options.strip_markdown_code_fence {
            return input;
        }

        let Some(opening_fence) = Self::opening_markdown_fence(input) else {
            return input;
        };
        let Some((line_end, content_start)) = Self::first_line_break(input) else {
            return input;
        };
        let opening_tag = input[opening_fence.marker_end..line_end].trim();
        if self.options.strip_markdown_code_fence_json_only
            && !Self::is_json_code_fence_tag(opening_tag)
        {
            return input;
        }

        let content = &input[content_start..];

        if let Some(without_close) = Self::strip_markdown_closing_fence(content, opening_fence) {
            return without_close;
        }
        if self.options.strip_markdown_code_fence_requires_closing {
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
            (Some(newline), Some(carriage_return)) if carriage_return < newline => {
                let content_start = if newline == carriage_return + 1 {
                    newline + 1
                } else {
                    carriage_return + 1
                };
                Some((carriage_return, content_start))
            }
            (Some(newline), _) => Some((newline, newline + 1)),
            (None, Some(carriage_return)) => Some((carriage_return, carriage_return + 1)),
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

    /// Removes a valid closing Markdown code fence from `content` when present.
    ///
    /// A closing fence is considered valid only when the last non-whitespace
    /// token is a backtick fence that is at least as long as the opening fence
    /// and appears on its own line.
    fn strip_markdown_closing_fence(content: &str, opening_fence: MarkdownFence) -> Option<&str> {
        let trimmed_end = content.trim_end_matches(char::is_whitespace);
        let closing_line_start = trimmed_end
            .rfind('\n')
            .or_else(|| trimmed_end.rfind('\r'))
            .map_or(0, |index| index + 1);
        let closing_line = trimmed_end[closing_line_start..].trim();
        let closing_len = Self::same_marker_fence_len(closing_line, opening_fence.marker)?;

        if closing_len == closing_line.len() && closing_len >= opening_fence.marker_len {
            Some(&content[..closing_line_start])
        } else {
            None
        }
    }

    /// Returns the marker run length when `line` starts with the same fence marker.
    fn same_marker_fence_len(line: &str, marker: u8) -> Option<usize> {
        let count = line.bytes().take_while(|byte| *byte == marker).count();
        (count >= 3).then_some(count)
    }

    /// Escapes raw ASCII control chars inside JSON string literals.
    ///
    /// Characters outside strings remain unchanged. Existing escape sequences are
    /// preserved so valid escapes are not double-escaped.
    fn escape_control_chars_in_json_strings<'a>(&self, input: &'a str) -> Cow<'a, str> {
        if !self.options.escape_control_chars_in_strings {
            return Cow::Borrowed(input);
        }

        let replacement_count = Self::count_control_chars_in_json_strings(input);
        if replacement_count == 0 {
            return Cow::Borrowed(input);
        }

        let mut in_string = false;
        let mut in_escape = false;
        let mut output = String::with_capacity(input.len() + replacement_count * 5);

        for ch in input.chars() {
            let mut replacement = None;

            if in_string {
                if in_escape {
                    in_escape = false;
                } else if ch == '\\' {
                    in_escape = true;
                } else if ch == '"' {
                    in_string = false;
                } else if ('\u{0000}'..='\u{001f}').contains(&ch) {
                    replacement = Some(self.escaped_control_char(ch));
                }
            } else if ch == '"' {
                in_string = true;
            }

            if let Some(escaped) = replacement {
                output.push_str(escaped);
                continue;
            }

            output.push(ch);
        }

        Cow::Owned(output)
    }

    /// Counts raw ASCII control chars inside JSON string literals.
    fn count_control_chars_in_json_strings(input: &str) -> usize {
        let mut in_string = false;
        let mut in_escape = false;
        let mut count = 0;

        for ch in input.chars() {
            if in_string {
                if in_escape {
                    in_escape = false;
                } else if ch == '\\' {
                    in_escape = true;
                } else if ch == '"' {
                    in_string = false;
                } else if ('\u{0000}'..='\u{001f}').contains(&ch) {
                    count += 1;
                }
            } else if ch == '"' {
                in_string = true;
            }
        }

        count
    }

    /// Maps one supported ASCII control character to its JSON escape.
    ///
    /// This helper only handles characters in `U+0000..=U+001F`.
    fn escaped_control_char(&self, ch: char) -> &'static str {
        match ch {
            '\u{0008}' => "\\b",
            '\u{0009}' => "\\t",
            '\u{000a}' => "\\n",
            '\u{000c}' => "\\f",
            '\u{000d}' => "\\r",
            '\u{0000}' => "\\u0000",
            '\u{0001}' => "\\u0001",
            '\u{0002}' => "\\u0002",
            '\u{0003}' => "\\u0003",
            '\u{0004}' => "\\u0004",
            '\u{0005}' => "\\u0005",
            '\u{0006}' => "\\u0006",
            '\u{0007}' => "\\u0007",
            '\u{000b}' => "\\u000b",
            '\u{000e}' => "\\u000e",
            '\u{000f}' => "\\u000f",
            '\u{0010}' => "\\u0010",
            '\u{0011}' => "\\u0011",
            '\u{0012}' => "\\u0012",
            '\u{0013}' => "\\u0013",
            '\u{0014}' => "\\u0014",
            '\u{0015}' => "\\u0015",
            '\u{0016}' => "\\u0016",
            '\u{0017}' => "\\u0017",
            '\u{0018}' => "\\u0018",
            '\u{0019}' => "\\u0019",
            '\u{001a}' => "\\u001a",
            '\u{001b}' => "\\u001b",
            '\u{001c}' => "\\u001c",
            '\u{001d}' => "\\u001d",
            '\u{001e}' => "\\u001e",
            '\u{001f}' => "\\u001f",
            _ => unreachable!("escaped_control_char only supports ASCII control chars"),
        }
    }
}
