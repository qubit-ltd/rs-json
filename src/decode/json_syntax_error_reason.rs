// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stable reasons reported by strict JSON lexical admission.

use std::fmt;

use crate::lexical::JsonLexicalErrorReason;

/// The concrete reason why a JSON document was rejected lexically.
///
/// This enum intentionally remains exhaustive so callers can classify every
/// documented lexical rejection at compile time. New reasons require a
/// breaking release rather than a `#[non_exhaustive]` change.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::JsonSyntaxErrorReason;
///
/// let reason = JsonSyntaxErrorReason::UnexpectedByte { byte: b'?' };
/// assert_eq!(reason.to_string(), "unexpected byte 0x3f");
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsonSyntaxErrorReason {
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
    /// Non-whitespace bytes follow the complete root value.
    TrailingCharacters,
    /// A nesting or position counter overflowed.
    NestingOverflow,
}

impl From<JsonLexicalErrorReason> for JsonSyntaxErrorReason {
    /// Exhaustively maps the shared lexical reason into the public text reason.
    #[inline]
    fn from(reason: JsonLexicalErrorReason) -> Self {
        match reason {
            JsonLexicalErrorReason::UnexpectedEnd => Self::UnexpectedEnd,
            JsonLexicalErrorReason::UnexpectedByte { byte } => Self::UnexpectedByte { byte },
            JsonLexicalErrorReason::ExpectedColon => Self::ExpectedColon,
            JsonLexicalErrorReason::ExpectedCommaOrArrayEnd => Self::ExpectedCommaOrArrayEnd,
            JsonLexicalErrorReason::ExpectedCommaOrObjectEnd => Self::ExpectedCommaOrObjectEnd,
            JsonLexicalErrorReason::ExpectedObjectKey => Self::ExpectedObjectKey,
            JsonLexicalErrorReason::InvalidEscape => Self::InvalidEscape,
            JsonLexicalErrorReason::InvalidUnicodeEscape => Self::InvalidUnicodeEscape,
            JsonLexicalErrorReason::UnpairedSurrogate => Self::UnpairedSurrogate,
            JsonLexicalErrorReason::InvalidUtf8 => Self::InvalidUtf8,
            JsonLexicalErrorReason::InvalidNumber => Self::InvalidNumber,
            JsonLexicalErrorReason::TrailingCharacters => Self::TrailingCharacters,
            JsonLexicalErrorReason::NestingOverflow => Self::NestingOverflow,
        }
    }
}

impl fmt::Display for JsonSyntaxErrorReason {
    /// Formats the stable human-readable reason.
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
            Self::TrailingCharacters => formatter.write_str("trailing characters"),
            Self::NestingOverflow => formatter.write_str("JSON nesting overflow"),
        }
    }
}
