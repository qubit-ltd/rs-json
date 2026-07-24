// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
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
/// replacement it lazily creates an output [`String`] and copies unchanged
/// UTF-8 byte ranges between replacements.
pub(super) struct ControlCharacterEscaper;

impl ControlCharacterEscaper {
    /// Scans the input for control-character escaping requirements.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON-like text to scan.
    /// * `enabled` - Whether raw control characters should be escaped.
    ///
    /// # Returns
    ///
    /// A tuple containing the repaired byte length and whether at least one
    /// replacement is required. The calculation does not allocate.
    #[must_use]
    pub(super) fn scan(input: &str, enabled: bool) -> (usize, bool) {
        if !enabled || !Self::contains_ascii_control(input) {
            return (input.len(), false);
        }

        let mut in_string = false;
        let mut in_escape = false;
        let mut normalized_len = input.len();
        let mut needs_escape = false;

        for byte in input.bytes() {
            if let Some(replacement) =
                Self::replacement(byte, &mut in_string, &mut in_escape)
            {
                needs_escape = true;
                normalized_len = normalized_len
                    .saturating_add(replacement.len().saturating_sub(1));
            }
        }

        (normalized_len, needs_escape)
    }

    /// Escapes raw C0 control characters in JSON string literals when enabled.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON-like text to scan.
    /// * `enabled` - Whether raw control characters should be escaped.
    ///
    /// # Returns
    ///
    /// Borrowed input when escaping is disabled or no replacement is needed,
    /// or owned rewritten text after the first replacement.
    #[must_use]
    pub(super) fn escape<'a>(input: &'a str, enabled: bool) -> Cow<'a, str> {
        if !enabled {
            return Cow::Borrowed(input);
        }
        if !Self::contains_ascii_control(input) {
            return Cow::Borrowed(input);
        }

        Self::rewrite(input, input.len() + 5)
    }

    /// Escapes input using results from a preceding [`Self::scan`] call.
    ///
    /// # Parameters
    ///
    /// * `input` - The same JSON-like text previously passed to [`Self::scan`].
    /// * `normalized_len` - Repaired byte length returned by [`Self::scan`].
    /// * `needs_escape` - Replacement flag returned by [`Self::scan`].
    ///
    /// # Returns
    ///
    /// Borrowed input when no replacement is needed, or owned rewritten text
    /// with exact preallocated capacity otherwise.
    #[must_use]
    pub(super) fn escape_with_scan<'a>(
        input: &'a str,
        normalized_len: usize,
        needs_escape: bool,
    ) -> Cow<'a, str> {
        if !needs_escape {
            return Cow::Borrowed(input);
        }

        Self::rewrite(input, normalized_len)
    }

    /// Reports whether the input contains at least one ASCII C0 byte.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON-like UTF-8 text to inspect without allocation.
    ///
    /// # Returns
    ///
    /// `true` when a byte is below `0x20`; otherwise `false`. UTF-8
    /// continuation bytes are never classified as C0 controls.
    #[inline]
    fn contains_ascii_control(input: &str) -> bool {
        const HIGH_BITS: u64 = 0x8080_8080_8080_8080;
        const CONTROL_OFFSET: u64 = 0x2020_2020_2020_2020;

        let (chunks, remainder) = input.as_bytes().as_chunks::<8>();
        chunks.iter().any(|chunk| {
            let bytes = u64::from_ne_bytes(*chunk);
            // Setting each high bit prevents cross-byte borrows; restoring the
            // original high bits keeps UTF-8 bytes out of the C0 range.
            let non_control_high_bits =
                ((bytes | HIGH_BITS).wrapping_sub(CONTROL_OFFSET) | bytes)
                    & HIGH_BITS;
            non_control_high_bits != HIGH_BITS
        }) || remainder.iter().any(|byte| *byte < 0x20)
    }

    /// Rewrites raw C0 controls using the requested initial capacity.
    ///
    /// # Parameters
    ///
    /// * `input` - JSON-like text known to contain a possible replacement.
    /// * `capacity` - Initial capacity for the lazily allocated output.
    ///
    /// # Returns
    ///
    /// Borrowed input when state-aware scanning finds no replacement, or owned
    /// rewritten text otherwise.
    fn rewrite<'a>(input: &'a str, capacity: usize) -> Cow<'a, str> {
        let mut in_string = false;
        let mut in_escape = false;
        let mut copy_start = 0;
        let mut output: Option<String> = None;

        for (index, byte) in input.bytes().enumerate() {
            let replacement =
                Self::replacement(byte, &mut in_string, &mut in_escape);
            let Some(replacement) = replacement else {
                continue;
            };

            let output =
                output.get_or_insert_with(|| String::with_capacity(capacity));
            let unchanged = &input[copy_start..index];
            match unchanged.as_bytes() {
                [] => {}
                [byte] => {
                    // A valid one-byte UTF-8 slice is necessarily ASCII.
                    output.push(char::from(*byte));
                }
                _ => output.push_str(unchanged),
            }
            output.push_str(replacement);
            copy_start = index + 1;
        }

        output.map_or_else(
            || Cow::Borrowed(input),
            |mut output| {
                output.push_str(&input[copy_start..]);
                Cow::Owned(output)
            },
        )
    }

    /// Returns the required replacement while advancing JSON-string state.
    ///
    /// # Parameters
    ///
    /// * `byte` - Current input byte.
    /// * `in_string` - Whether the scanner is currently inside a JSON string.
    /// * `in_escape` - Whether an unmatched backslash precedes `byte`.
    ///
    /// # Returns
    ///
    /// `Some(replacement)` when `byte` is a raw C0 control character
    /// requiring repair, or `None` when it should be copied unchanged.
    fn replacement(
        byte: u8,
        in_string: &mut bool,
        in_escape: &mut bool,
    ) -> Option<&'static str> {
        if *in_string {
            if *in_escape {
                *in_escape = false;
                return Self::escaped_control_byte(byte)
                    .map(|escape| &escape[1..]);
            }
            if byte == b'\\' {
                *in_escape = true;
            } else if byte == b'"' {
                *in_string = false;
            } else {
                return Self::escaped_control_byte(byte);
            }
        } else if byte == b'"' {
            *in_string = true;
        }
        None
    }

    /// Maps an ASCII C0 control character to its JSON escape.
    ///
    /// # Parameters
    ///
    /// * `byte` - Byte to map.
    ///
    /// # Returns
    ///
    /// `Some(escape)` for `0x00..=0x1f`, or `None` for other bytes.
    fn escaped_control_byte(byte: u8) -> Option<&'static str> {
        match byte {
            b'\x08' => Some("\\b"),
            b'\x09' => Some("\\t"),
            b'\x0a' => Some("\\n"),
            b'\x0c' => Some("\\f"),
            b'\x0d' => Some("\\r"),
            b'\x00' => Some("\\u0000"),
            b'\x01' => Some("\\u0001"),
            b'\x02' => Some("\\u0002"),
            b'\x03' => Some("\\u0003"),
            b'\x04' => Some("\\u0004"),
            b'\x05' => Some("\\u0005"),
            b'\x06' => Some("\\u0006"),
            b'\x07' => Some("\\u0007"),
            b'\x0b' => Some("\\u000b"),
            b'\x0e' => Some("\\u000e"),
            b'\x0f' => Some("\\u000f"),
            b'\x10' => Some("\\u0010"),
            b'\x11' => Some("\\u0011"),
            b'\x12' => Some("\\u0012"),
            b'\x13' => Some("\\u0013"),
            b'\x14' => Some("\\u0014"),
            b'\x15' => Some("\\u0015"),
            b'\x16' => Some("\\u0016"),
            b'\x17' => Some("\\u0017"),
            b'\x18' => Some("\\u0018"),
            b'\x19' => Some("\\u0019"),
            b'\x1a' => Some("\\u001a"),
            b'\x1b' => Some("\\u001b"),
            b'\x1c' => Some("\\u001c"),
            b'\x1d' => Some("\\u001d"),
            b'\x1e' => Some("\\u001e"),
            b'\x1f' => Some("\\u001f"),
            _ => None,
        }
    }
}
