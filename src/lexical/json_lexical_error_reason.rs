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
            Self::TrailingCharacters => formatter.write_str("trailing characters"),
            Self::NestingOverflow => formatter.write_str("JSON nesting overflow"),
        }
    }
}
