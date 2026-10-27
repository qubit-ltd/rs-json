//! Fallible callbacks for the shared lexical JSON scanner.

use super::JsonLexicalEvent;

pub(crate) trait JsonLexicalConsumer<'input> {
    type Error;

    fn before_value(&mut self, depth: usize, offset: usize) -> Result<(), Self::Error>;
    fn before_key(&mut self, offset: usize) -> Result<(), Self::Error>;
    fn before_number(&mut self, token: &'input str, offset: usize) -> Result<(), Self::Error>;
    fn event(&mut self, event: JsonLexicalEvent<'input>) -> Result<(), Self::Error>;
}

pub(crate) enum JsonLexicalScanPoint<'input> {
    BeforeValue { depth: usize, offset: usize },
    BeforeKey { offset: usize },
    BeforeNumber { token: &'input str, offset: usize },
    Event(JsonLexicalEvent<'input>),
}
