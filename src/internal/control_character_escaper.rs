// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private raw-control-character escaper.

use std::borrow::Cow;

/// Escapes raw ASCII control characters occurring within JSON strings.
///
/// The scanner borrows its input unless it finds a replacement. On the first
/// replacement it allocates once, copies the already-scanned prefix, and
/// appends all remaining transformed characters.
pub(crate) struct ControlCharacterEscaper;

impl ControlCharacterEscaper {
    /// Escapes raw C0 control characters in JSON string literals when enabled.
    #[must_use]
    pub(crate) fn escape<'a>(input: &'a str, enabled: bool) -> Cow<'a, str> {
        if !enabled {
            return Cow::Borrowed(input);
        }

        let mut in_string = false;
        let mut in_escape = false;
        let mut output: Option<String> = None;

        for (index, ch) in input.char_indices() {
            let replacement =
                Self::replacement(ch, &mut in_string, &mut in_escape);
            let Some(replacement) = replacement else {
                if let Some(output) = output.as_mut() {
                    output.push(ch);
                }
                continue;
            };

            let output = output.get_or_insert_with(|| {
                let mut result = String::with_capacity(input.len() + 5);
                result.push_str(&input[..index]);
                result
            });
            output.push_str(replacement);
        }

        output.map_or_else(|| Cow::Borrowed(input), Cow::Owned)
    }

    /// Returns the required replacement while advancing JSON-string state.
    fn replacement(
        ch: char,
        in_string: &mut bool,
        in_escape: &mut bool,
    ) -> Option<&'static str> {
        if *in_string {
            if *in_escape {
                *in_escape = false;
                return Self::escaped_control_char(ch)
                    .map(|escape| &escape[1..]);
            }
            if ch == '\\' {
                *in_escape = true;
            } else if ch == '"' {
                *in_string = false;
            } else {
                return Self::escaped_control_char(ch);
            }
        } else if ch == '"' {
            *in_string = true;
        }
        None
    }

    /// Maps an ASCII C0 control character to its JSON escape.
    fn escaped_control_char(ch: char) -> Option<&'static str> {
        match ch {
            '\u{0008}' => Some("\\b"),
            '\u{0009}' => Some("\\t"),
            '\u{000a}' => Some("\\n"),
            '\u{000c}' => Some("\\f"),
            '\u{000d}' => Some("\\r"),
            '\u{0000}' => Some("\\u0000"),
            '\u{0001}' => Some("\\u0001"),
            '\u{0002}' => Some("\\u0002"),
            '\u{0003}' => Some("\\u0003"),
            '\u{0004}' => Some("\\u0004"),
            '\u{0005}' => Some("\\u0005"),
            '\u{0006}' => Some("\\u0006"),
            '\u{0007}' => Some("\\u0007"),
            '\u{000b}' => Some("\\u000b"),
            '\u{000e}' => Some("\\u000e"),
            '\u{000f}' => Some("\\u000f"),
            '\u{0010}' => Some("\\u0010"),
            '\u{0011}' => Some("\\u0011"),
            '\u{0012}' => Some("\\u0012"),
            '\u{0013}' => Some("\\u0013"),
            '\u{0014}' => Some("\\u0014"),
            '\u{0015}' => Some("\\u0015"),
            '\u{0016}' => Some("\\u0016"),
            '\u{0017}' => Some("\\u0017"),
            '\u{0018}' => Some("\\u0018"),
            '\u{0019}' => Some("\\u0019"),
            '\u{001a}' => Some("\\u001a"),
            '\u{001b}' => Some("\\u001b"),
            '\u{001c}' => Some("\\u001c"),
            '\u{001d}' => Some("\\u001d"),
            '\u{001e}' => Some("\\u001e"),
            '\u{001f}' => Some("\\u001f"),
            _ => None,
        }
    }
}
