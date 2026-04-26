/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Internal normalization utilities used by the lenient JSON decoder.
//!
//! Author: Haixing Hu

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
    /// input and has at least three backticks. If a valid closing fence is
    /// present after trimming the trailing side, it is also removed.
    fn strip_markdown_code_fence<'a>(&self, input: &'a str) -> &'a str {
        if !self.options.strip_markdown_code_fence {
            return input;
        }

        let Some(opening_fence_len) = Self::opening_backtick_fence_len(input) else {
            return input;
        };
        let Some(line_end) = input.find('\n') else {
            return input;
        };
        let opening_tag = input[opening_fence_len..line_end].trim();
        if self.options.strip_markdown_code_fence_json_only
            && !Self::is_json_code_fence_tag(opening_tag)
        {
            return input;
        }

        let content = &input[line_end + 1..];

        if let Some(without_close) = Self::strip_markdown_closing_fence(content, opening_fence_len)
        {
            return without_close;
        }
        if self.options.strip_markdown_code_fence_requires_closing {
            input
        } else {
            content
        }
    }

    /// Returns the byte length of an opening backtick fence when present.
    fn opening_backtick_fence_len(input: &str) -> Option<usize> {
        let count = input.bytes().take_while(|byte| *byte == b'`').count();
        (count >= 3).then_some(count)
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
    fn strip_markdown_closing_fence(content: &str, opening_fence_len: usize) -> Option<&str> {
        let trimmed_end = content.trim_end_matches(char::is_whitespace);
        let closing_line_start = trimmed_end
            .rfind('\n')
            .or_else(|| trimmed_end.rfind('\r'))
            .map_or(0, |index| index + 1);
        let closing_line = trimmed_end[closing_line_start..].trim();
        let closing_len = Self::opening_backtick_fence_len(closing_line)?;

        if closing_len == closing_line.len() && closing_len >= opening_fence_len {
            Some(&content[..closing_line_start])
        } else {
            None
        }
    }

    /// Escapes raw ASCII control chars inside JSON string literals.
    ///
    /// Characters outside strings remain unchanged. Existing escape sequences are
    /// preserved so valid escapes are not double-escaped.
    fn escape_control_chars_in_json_strings<'a>(&self, input: &'a str) -> Cow<'a, str> {
        if !self.options.escape_control_chars_in_strings {
            return Cow::Borrowed(input);
        }

        let mut in_string = false;
        let mut in_escape = false;
        let mut output: Option<String> = None;

        for (index, ch) in input.char_indices() {
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
                let text = output.get_or_insert_with(|| {
                    let mut text = String::with_capacity(input.len() + 8);
                    text.push_str(&input[..index]);
                    text
                });
                text.push_str(escaped);
                continue;
            }

            if let Some(text) = output.as_mut() {
                text.push(ch);
            }
        }

        match output {
            Some(text) => Cow::Owned(text),
            None => Cow::Borrowed(input),
        }
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
