// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private representation of a lenient Markdown opening fence.

use super::super::MarkdownFenceClosing;
use super::super::MarkdownFencePolicy;

/// Describes one recognized Markdown code-fence opening line.
#[derive(Debug, Clone, Copy)]
pub(super) struct MarkdownFence {
    /// Stores the byte marker used by the fence.
    marker: u8,
    /// Stores the number of repeated marker bytes in the opening fence.
    marker_len: usize,
    /// Stores the byte index immediately after the opening marker run.
    marker_end: usize,
}

impl MarkdownFence {
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
    fn parse_opening(input: &str) -> Option<Self> {
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
        (marker_len >= 3).then_some(Self {
            marker,
            marker_len,
            marker_end: indent_len + marker_len,
        })
    }

    /// Removes one supported outer Markdown code fence when configured.
    ///
    /// # Parameters
    ///
    /// * `input` - Text to inspect for one outer Markdown code fence.
    /// * `policy` - Fence recognition and closing-fence policy.
    ///
    /// # Returns
    ///
    /// The fenced body when the active policy accepts the fence, or the
    /// unchanged input otherwise.
    #[must_use]
    pub(super) fn strip_outer<'a>(input: &'a str, policy: &MarkdownFencePolicy) -> &'a str {
        let (json_only, closing) = match policy {
            MarkdownFencePolicy::Disabled => return input,
            MarkdownFencePolicy::Any { closing } => (false, *closing),
            MarkdownFencePolicy::JsonOnly { closing } => (true, *closing),
        };
        let Some(opening_fence) = Self::parse_opening(input) else {
            return input;
        };
        let Some((line_end, content_start)) = Self::first_line_break(input) else {
            return input;
        };
        let info_string = input[opening_fence.marker_end..line_end].trim();
        if json_only && !Self::is_json_info_string(info_string) {
            return input;
        }
        let content = &input[content_start..];
        if let Some(without_close) = opening_fence.strip_closing(content) {
            return without_close;
        }
        if closing == MarkdownFenceClosing::Required {
            input
        } else {
            content
        }
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
        let bytes = input.as_bytes();
        let line_end = bytes
            .iter()
            .position(|byte| matches!(*byte, b'\n' | b'\r'))?;
        let next_line_start = if bytes[line_end] == b'\r' && bytes.get(line_end + 1) == Some(&b'\n')
        {
            line_end + 2
        } else {
            line_end + 1
        };
        Some((line_end, next_line_start))
    }

    /// Returns whether a fenced info string should be treated as JSON.
    ///
    /// # Parameters
    ///
    /// * `info_string` - Markdown fence info string.
    ///
    /// # Returns
    ///
    /// `true` for an empty, `json`, or `jsonc` first token, ignoring ASCII
    /// case; otherwise, `false`. The `jsonc` token is accepted only as a fence
    /// label and does not enable non-standard JSON grammar.
    #[inline]
    #[must_use]
    fn is_json_info_string(info_string: &str) -> bool {
        let language = info_string.split_whitespace().next().unwrap_or("");
        language.is_empty()
            || language.eq_ignore_ascii_case("json")
            || language.eq_ignore_ascii_case("jsonc")
    }

    /// Removes a compatible closing fence from content when present.
    ///
    /// # Parameters
    ///
    /// * `content` - Fenced body and possible closing fence.
    ///
    /// # Returns
    ///
    /// `Some(body)` when, after ignoring trailing ASCII spaces, tabs, CRs, and
    /// LFs, the final line begins with zero to three ASCII spaces and contains
    /// a compatible closing marker run; otherwise, `None`.
    fn strip_closing<'a>(&self, content: &'a str) -> Option<&'a str> {
        let trimmed_end = content.trim_end_matches([' ', '\t', '\n', '\r']);
        let closing_line_start = trimmed_end
            .bytes()
            .rposition(|byte| matches!(byte, b'\n' | b'\r'))
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
        let closing_len = self.matching_marker_len(marker_line)?;
        if closing_len == marker_line.len() && closing_len >= self.marker_len {
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
    ///
    /// # Returns
    ///
    /// `Some(length)` for a run of at least three matching marker bytes, or
    /// `None` otherwise.
    fn matching_marker_len(&self, line: &str) -> Option<usize> {
        let count = line.bytes().take_while(|byte| *byte == self.marker).count();
        (count >= 3).then_some(count)
    }
}
