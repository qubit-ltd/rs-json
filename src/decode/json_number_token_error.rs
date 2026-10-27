//! Errors from exact-safe-integer conversion.

/// Why a validated number cannot be represented as a safe integer.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonNumberTokenError {
    /// The token has a non-integral mathematical value.
    Fractional,
    /// The integer is outside the inclusive safe-integer range.
    OutOfSafeIntegerRange,
}

impl std::fmt::Display for JsonNumberTokenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fractional => formatter.write_str("number is fractional"),
            Self::OutOfSafeIntegerRange => formatter.write_str("integer is outside the safe range"),
        }
    }
}

impl std::error::Error for JsonNumberTokenError {}
