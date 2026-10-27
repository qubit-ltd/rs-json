//! Number representations accepted by JSON decoding.

/// Controls conversion of syntactically valid JSON number tokens.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JsonDecodeNumberModel {
    /// Accept signed and unsigned 64-bit integers and finite floating values.
    #[default]
    Integer64,
    /// Also represent out-of-range integer tokens as finite `f64` values.
    Integer64WithFiniteFallback,
}
