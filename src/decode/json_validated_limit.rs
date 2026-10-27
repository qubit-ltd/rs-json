//! Additional validated decoding limits.

/// Identifies an additional per-document decode limit.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonValidatedLimit {
    /// Value entries plus decoded object-key entries.
    ValueAndKeyNodes,
    /// Source bytes inside one encoded string token, excluding quotes.
    EncodedStringBytes,
}
