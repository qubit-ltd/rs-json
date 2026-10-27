//! Contexts for immediate validated decode failures.

/// Grammar location where a validated decode failure occurred.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeContext {
    /// A JSON value token was expected.
    Value,
    /// An object key token was expected.
    ObjectKey,
    /// A string token was being scanned.
    String,
    /// A number token was being scanned or projected.
    Number,
    /// An object colon was expected.
    Colon,
    /// An array separator or closing bracket was expected.
    ArraySeparator,
    /// An object separator or closing brace was expected.
    ObjectSeparator,
    /// The document had trailing input after its root value.
    TrailingInput,
}
