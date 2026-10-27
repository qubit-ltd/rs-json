//! Errors returned by fallible lexical scanning.

use std::fmt;

use qubit_budget::ResourceQuantity;

use super::JsonLexicalError;

#[derive(Debug)]
pub(crate) enum JsonLexicalScanError<E, R, Q>
where
    Q: ResourceQuantity,
{
    Lexical(JsonLexicalError<R, Q>),
    Consumer(E),
    EncodedStringLimit {
        maximum: usize,
        observed: usize,
        offset: usize,
    },
}

impl<E, R, Q> From<JsonLexicalError<R, Q>> for JsonLexicalScanError<E, R, Q>
where
    Q: ResourceQuantity,
{
    fn from(error: JsonLexicalError<R, Q>) -> Self {
        Self::Lexical(error)
    }
}

impl<E, R, Q> fmt::Display for JsonLexicalScanError<E, R, Q>
where
    E: fmt::Display,
    R: fmt::Debug,
    Q: ResourceQuantity,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lexical(error) => write!(f, "{error:?}"),
            Self::Consumer(error) => error.fmt(f),
            Self::EncodedStringLimit { .. } => f.write_str("encoded JSON string limit exceeded"),
        }
    }
}
