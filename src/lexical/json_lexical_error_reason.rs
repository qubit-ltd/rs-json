// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stable reasons produced by domain-independent lexical JSON scanning.

use std::fmt;

/// Stable reason for a domain-independent lexical JSON rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JsonLexicalErrorReason {
    /// The document ended before a complete token or container was found.
    UnexpectedEnd,
    /// A byte is not valid at the current JSON position.
    UnexpectedByte {
        /// The unexpected byte.
        byte: u8,
    },
    /// An object key was not followed by a colon.
    ExpectedColon,
    /// An array value was not followed by a comma or closing bracket.
    ExpectedCommaOrArrayEnd,
    /// An object value was not followed by a comma or closing brace.
    ExpectedCommaOrObjectEnd,
    /// An object key was expected at the current position.
    ExpectedObjectKey,
    /// A string escape sequence is invalid.
    InvalidEscape,
    /// A Unicode escape does not contain four hexadecimal digits.
    InvalidUnicodeEscape,
    /// A Unicode surrogate pair is malformed.
    UnpairedSurrogate,
    /// The input contains invalid UTF-8.
    InvalidUtf8,
    /// A number does not follow JSON number grammar.
    InvalidNumber,
    /// An integer is outside the supported `i64`/`u64` range.
    IntegerOutOfRange,
    /// A fractional or exponential number is outside finite `f64` range.
    FloatOutOfRange,
    /// Non-whitespace bytes follow the complete root value.
    TrailingCharacters,
    /// A nesting or position counter overflowed.
    NestingOverflow,
}

impl fmt::Display for JsonLexicalErrorReason {
    /// Formats the stable human-readable reason without input-derived text.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => formatter.write_str("unexpected end of input"),
            Self::UnexpectedByte { byte } => {
                write!(formatter, "unexpected byte 0x{byte:02x}")
            }
            Self::ExpectedColon => formatter.write_str("expected ':'"),
            Self::ExpectedCommaOrArrayEnd => formatter.write_str("expected ',' or ']' in array"),
            Self::ExpectedCommaOrObjectEnd => formatter.write_str("expected ',' or '}' in object"),
            Self::ExpectedObjectKey => formatter.write_str("expected object key"),
            Self::InvalidEscape => formatter.write_str("invalid string escape"),
            Self::InvalidUnicodeEscape => formatter.write_str("invalid Unicode escape"),
            Self::UnpairedSurrogate => formatter.write_str("unpaired Unicode surrogate"),
            Self::InvalidUtf8 => formatter.write_str("invalid UTF-8"),
            Self::InvalidNumber => formatter.write_str("invalid JSON number"),
            Self::IntegerOutOfRange => formatter.write_str("JSON integer is outside the supported 64-bit range"),
            Self::FloatOutOfRange => formatter.write_str("JSON number is outside the finite f64 range"),
            Self::TrailingCharacters => formatter.write_str("trailing characters"),
            Self::NestingOverflow => formatter.write_str("JSON nesting overflow"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JsonLexicalErrorReason;

    /// Verifies every stable lexical rejection reason has a deterministic
    /// human-readable representation.
    #[test]
    fn test_json_lexical_error_reason_formats_every_variant() {
        let cases = [
            (JsonLexicalErrorReason::UnexpectedEnd, "unexpected end of input"),
            (
                JsonLexicalErrorReason::UnexpectedByte { byte: 0x1f },
                "unexpected byte 0x1f",
            ),
            (JsonLexicalErrorReason::ExpectedColon, "expected ':'"),
            (
                JsonLexicalErrorReason::ExpectedCommaOrArrayEnd,
                "expected ',' or ']' in array",
            ),
            (
                JsonLexicalErrorReason::ExpectedCommaOrObjectEnd,
                "expected ',' or '}' in object",
            ),
            (JsonLexicalErrorReason::ExpectedObjectKey, "expected object key"),
            (JsonLexicalErrorReason::InvalidEscape, "invalid string escape"),
            (JsonLexicalErrorReason::InvalidUnicodeEscape, "invalid Unicode escape"),
            (JsonLexicalErrorReason::UnpairedSurrogate, "unpaired Unicode surrogate"),
            (JsonLexicalErrorReason::InvalidUtf8, "invalid UTF-8"),
            (JsonLexicalErrorReason::InvalidNumber, "invalid JSON number"),
            (
                JsonLexicalErrorReason::IntegerOutOfRange,
                "JSON integer is outside the supported 64-bit range",
            ),
            (
                JsonLexicalErrorReason::FloatOutOfRange,
                "JSON number is outside the finite f64 range",
            ),
            (JsonLexicalErrorReason::TrailingCharacters, "trailing characters"),
            (JsonLexicalErrorReason::NestingOverflow, "JSON nesting overflow"),
        ];

        for (reason, expected) in cases {
            assert_eq!(reason.to_string(), expected);
        }
    }
}
