//! Number handling choices for validated decoding.

/// Selects how a validated source number is represented in the output tree.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonNumberPolicy {
    /// Preserve the number according to the configured decode model.
    Preserve,
    /// Require an exact integer in JavaScript's safe integer range.
    ExactSafeInteger,
}
