// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Byte cursor for iterative lexical JSON scanning.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;

use super::json_lexical_container_frame::JsonLexicalContainerFrame;
use super::json_lexical_error::JsonLexicalError;
use super::json_lexical_error_reason::JsonLexicalErrorReason;
use super::json_lexical_failure::JsonLexicalFailure;

/// Iterative cursor over the JSON bytes being admitted.
pub(super) struct JsonLexicalCursor<'input, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Complete JSON input.
    input: &'input [u8],
    /// Current input position.
    offset: usize,
    /// Value transaction charged by lexical admission.
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    /// Whether native measurements must be staged in the transaction.
    has_value_limits: bool,
}

impl<'input, 'transaction, 'budget, R, Q> JsonLexicalCursor<'input, 'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a cursor positioned at the beginning of `input`.
    #[inline(always)]
    pub(super) const fn new(
        input: &'input [u8],
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
        has_value_limits: bool,
    ) -> Self {
        Self {
            input,
            offset: 0,
            transaction,
            has_value_limits,
        }
    }

    /// Stages one native measurement when value limits are configured.
    fn admit(&mut self, measurement: JsonMeasurement) -> Result<(), JsonLexicalError<R, Q>> {
        if !self.has_value_limits {
            return Ok(());
        }
        self.transaction.try_admit(measurement).map_err(JsonLexicalError::from)
    }

    /// Enters one container when value limits are configured.
    fn enter_container(&mut self, kind: JsonContainerKind, depth: usize) -> Result<(), JsonLexicalError<R, Q>> {
        if !self.has_value_limits {
            return Ok(());
        }
        self.transaction
            .try_enter_container(kind, depth)
            .map_err(JsonLexicalError::from)
    }

    /// Checks one observed container count when value limits are configured.
    fn check_container_count(&mut self, kind: JsonContainerKind, count: usize) -> Result<(), JsonLexicalError<R, Q>> {
        if !self.has_value_limits {
            return Ok(());
        }
        self.transaction
            .check_container_count(kind, count)
            .map_err(JsonLexicalError::from)
    }

    /// Returns whether the cursor has consumed the complete input.
    #[must_use]
    #[inline(always)]
    pub(super) fn is_at_end(&self) -> bool {
        self.offset == self.input.len()
    }

    /// Advances past JSON whitespace.
    pub(super) fn skip_whitespace(&mut self) {
        let tail = &self.input[self.offset..];
        let skipped = tail
            .iter()
            .position(|byte| !matches!(*byte, b' ' | b'\n' | b'\r' | b'\t'))
            .unwrap_or(tail.len());
        self.offset += skipped;
    }

    /// Returns the current byte without advancing.
    #[must_use]
    #[inline(always)]
    fn peek(&self) -> Option<u8> {
        self.input.get(self.offset).copied()
    }

    /// Builds a structured syntax error at the current cursor position.
    pub(super) fn syntax(&self, reason: JsonLexicalErrorReason) -> JsonLexicalError<R, Q> {
        let (line, column) = self.line_column();
        JsonLexicalError::Syntax(JsonLexicalFailure {
            offset: self.offset,
            line,
            column,
            reason,
        })
    }

    /// Admits one JSON value and schedules any container continuation.
    pub(super) fn value(
        &mut self,
        depth: usize,
        stack: &mut Vec<JsonLexicalContainerFrame>,
    ) -> Result<(), JsonLexicalError<R, Q>> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => {
                self.offset += 1;
                self.enter_container(JsonContainerKind::Map, depth)?;
                stack.push(JsonLexicalContainerFrame::ObjectKey { depth, entries: 0 });
                Ok(())
            }
            Some(b'[') => {
                self.offset += 1;
                self.enter_container(JsonContainerKind::Sequence, depth)?;
                stack.push(JsonLexicalContainerFrame::ArrayValue { depth, items: 0 });
                Ok(())
            }
            Some(b'"') => {
                let bytes = self.string_bytes()?;
                self.admit(JsonMeasurement::String { depth, bytes })
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.offset;
                let bytes = self.number_bytes()?;
                self.admit(JsonMeasurement::Number { depth, bytes })?;
                self.validate_number_range(start, self.offset)
            }
            Some(b't') => self.literal(b"true", JsonMeasurement::Boolean { depth }),
            Some(b'f') => self.literal(b"false", JsonMeasurement::Boolean { depth }),
            Some(b'n') => self.literal(b"null", JsonMeasurement::Null { depth }),
            None => Err(self.syntax(JsonLexicalErrorReason::UnexpectedEnd)),
            Some(byte) => Err(self.syntax(JsonLexicalErrorReason::UnexpectedByte { byte })),
        }
    }

    /// Charges and consumes one scalar literal.
    fn literal(&mut self, literal: &[u8], measurement: JsonMeasurement) -> Result<(), JsonLexicalError<R, Q>> {
        if !self.input[self.offset..].starts_with(literal) {
            return Err(match self.peek() {
                None => self.syntax(JsonLexicalErrorReason::UnexpectedEnd),
                Some(byte) => self.syntax(JsonLexicalErrorReason::UnexpectedByte { byte }),
            });
        }
        let end = self.offset.saturating_add(literal.len());
        if !Self::is_value_delimiter(self.input.get(end).copied()) {
            self.offset = end;
            return Err(self.syntax(JsonLexicalErrorReason::UnexpectedByte {
                byte: self.peek().unwrap_or_default(),
            }));
        }
        self.admit(measurement)?;
        self.offset = end;
        Ok(())
    }

    /// Resumes a container after its child value has completed.
    pub(super) fn resume(
        &mut self,
        frame: JsonLexicalContainerFrame,
        stack: &mut Vec<JsonLexicalContainerFrame>,
    ) -> Result<(), JsonLexicalError<R, Q>> {
        match frame {
            JsonLexicalContainerFrame::ArrayValue { depth, items } => {
                self.skip_whitespace();
                if self.peek() == Some(b']') {
                    if items == 0 {
                        self.offset += 1;
                        return Ok(());
                    }
                    return Err(self.syntax(JsonLexicalErrorReason::UnexpectedByte {
                        byte: self.peek().unwrap_or_default(),
                    }));
                }
                let items = items
                    .checked_add(1)
                    .ok_or_else(|| self.syntax(JsonLexicalErrorReason::NestingOverflow))?;
                self.check_container_count(JsonContainerKind::Sequence, items)?;
                stack.push(JsonLexicalContainerFrame::ArrayDelimiter { depth, items });
                self.value(
                    depth
                        .checked_add(1)
                        .ok_or_else(|| self.syntax(JsonLexicalErrorReason::NestingOverflow))?,
                    stack,
                )
            }
            JsonLexicalContainerFrame::ArrayDelimiter { depth, items } => {
                self.skip_whitespace();
                match self.peek() {
                    Some(b',') => {
                        self.offset += 1;
                        stack.push(JsonLexicalContainerFrame::ArrayValue { depth, items });
                        Ok(())
                    }
                    Some(b']') => {
                        self.offset += 1;
                        Ok(())
                    }
                    None => Err(self.syntax(JsonLexicalErrorReason::UnexpectedEnd)),
                    Some(_) => Err(self.syntax(JsonLexicalErrorReason::ExpectedCommaOrArrayEnd)),
                }
            }
            JsonLexicalContainerFrame::ObjectKey { depth, entries } => {
                self.skip_whitespace();
                if self.peek() == Some(b'}') {
                    if entries == 0 {
                        self.offset += 1;
                        return Ok(());
                    }
                    return Err(self.syntax(JsonLexicalErrorReason::UnexpectedByte {
                        byte: self.peek().unwrap_or_default(),
                    }));
                }
                if self.peek() != Some(b'"') {
                    return Err(self.syntax(JsonLexicalErrorReason::ExpectedObjectKey));
                }
                let entries = entries
                    .checked_add(1)
                    .ok_or_else(|| self.syntax(JsonLexicalErrorReason::NestingOverflow))?;
                self.check_container_count(JsonContainerKind::Map, entries)?;
                let bytes = self.string_bytes()?;
                self.admit(JsonMeasurement::Key { bytes })?;
                self.skip_whitespace();
                if self.peek() != Some(b':') {
                    return Err(match self.peek() {
                        None => self.syntax(JsonLexicalErrorReason::UnexpectedEnd),
                        Some(_) => self.syntax(JsonLexicalErrorReason::ExpectedColon),
                    });
                }
                self.offset += 1;
                stack.push(JsonLexicalContainerFrame::ObjectDelimiter { depth, entries });
                self.value(
                    depth
                        .checked_add(1)
                        .ok_or_else(|| self.syntax(JsonLexicalErrorReason::NestingOverflow))?,
                    stack,
                )
            }
            JsonLexicalContainerFrame::ObjectDelimiter { depth, entries } => {
                self.skip_whitespace();
                match self.peek() {
                    Some(b',') => {
                        self.offset += 1;
                        stack.push(JsonLexicalContainerFrame::ObjectKey { depth, entries });
                        Ok(())
                    }
                    Some(b'}') => {
                        self.offset += 1;
                        Ok(())
                    }
                    None => Err(self.syntax(JsonLexicalErrorReason::UnexpectedEnd)),
                    Some(_) => Err(self.syntax(JsonLexicalErrorReason::ExpectedCommaOrObjectEnd)),
                }
            }
        }
    }

    /// Consumes one JSON string and returns its decoded UTF-8 byte length.
    fn string_bytes(&mut self) -> Result<usize, JsonLexicalError<R, Q>> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.offset += 1;
        let mut decoded = 0_usize;
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.offset += 1;
                    return Ok(decoded);
                }
                Some(b'\\') => {
                    self.offset += 1;
                    let bytes = match self.peek() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.offset += 1;
                            1
                        }
                        Some(b'u') => self.unicode_escape_bytes()?,
                        None => {
                            return Err(self.syntax(JsonLexicalErrorReason::UnexpectedEnd));
                        }
                        Some(_) => {
                            return Err(self.syntax(JsonLexicalErrorReason::InvalidEscape));
                        }
                    };
                    decoded = decoded
                        .checked_add(bytes)
                        .ok_or_else(|| self.syntax(JsonLexicalErrorReason::NestingOverflow))?;
                }
                Some(0x20..=0x7F) => {
                    let start = self.offset;
                    while let Some(byte) = self.peek()
                        && (0x20..=0x7F).contains(&byte)
                        && byte != b'"'
                        && byte != b'\\'
                    {
                        self.offset += 1;
                    }
                    decoded = decoded
                        .checked_add(self.offset - start)
                        .ok_or_else(|| self.syntax(JsonLexicalErrorReason::NestingOverflow))?;
                }
                Some(byte) if byte >= 0x80 => {
                    let width =
                        Self::utf8_width(byte).ok_or_else(|| self.syntax(JsonLexicalErrorReason::InvalidUtf8))?;
                    let end = self
                        .offset
                        .checked_add(width)
                        .ok_or_else(|| self.syntax(JsonLexicalErrorReason::NestingOverflow))?;
                    let text = self
                        .input
                        .get(self.offset..end)
                        .ok_or_else(|| self.syntax(JsonLexicalErrorReason::UnexpectedEnd))?;
                    let character = std::str::from_utf8(text)
                        .ok()
                        .and_then(|text| text.chars().next())
                        .filter(|character| character.len_utf8() == width)
                        .ok_or_else(|| self.syntax(JsonLexicalErrorReason::InvalidUtf8))?;
                    self.offset = end;
                    decoded = decoded
                        .checked_add(character.len_utf8())
                        .ok_or_else(|| self.syntax(JsonLexicalErrorReason::NestingOverflow))?;
                }
                None => {
                    return Err(self.syntax(JsonLexicalErrorReason::UnexpectedEnd));
                }
                Some(byte) => {
                    return Err(self.syntax(JsonLexicalErrorReason::UnexpectedByte { byte }));
                }
            }
        }
    }

    /// Consumes a Unicode escape and returns its decoded UTF-8 byte length.
    fn unicode_escape_bytes(&mut self) -> Result<usize, JsonLexicalError<R, Q>> {
        debug_assert_eq!(self.peek(), Some(b'u'));
        self.offset += 1;
        let first = self.hex_quad()?;
        let scalar = if (0xD800..=0xDBFF).contains(&first) {
            if self.input.get(self.offset..self.offset.saturating_add(2)) != Some(b"\\u") {
                return Err(match self.peek() {
                    None => self.syntax(JsonLexicalErrorReason::UnexpectedEnd),
                    Some(_) => self.syntax(JsonLexicalErrorReason::UnpairedSurrogate),
                });
            }
            self.offset += 2;
            let second = self.hex_quad()?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err(self.syntax(JsonLexicalErrorReason::UnpairedSurrogate));
            }
            0x1_0000 + ((u32::from(first) - 0xD800) << 10) + (u32::from(second) - 0xDC00)
        } else {
            if (0xDC00..=0xDFFF).contains(&first) {
                return Err(self.syntax(JsonLexicalErrorReason::UnpairedSurrogate));
            }
            u32::from(first)
        };
        char::from_u32(scalar)
            .map(char::len_utf8)
            .ok_or_else(|| self.syntax(JsonLexicalErrorReason::UnpairedSurrogate))
    }

    /// Consumes four hexadecimal digits from a Unicode escape.
    fn hex_quad(&mut self) -> Result<u16, JsonLexicalError<R, Q>> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let digit = match self.peek() {
                Some(byte @ b'0'..=b'9') => u16::from(byte - b'0'),
                Some(byte @ b'a'..=b'f') => u16::from(byte - b'a' + 10),
                Some(byte @ b'A'..=b'F') => u16::from(byte - b'A' + 10),
                None => {
                    return Err(self.syntax(JsonLexicalErrorReason::UnexpectedEnd));
                }
                Some(_) => {
                    return Err(self.syntax(JsonLexicalErrorReason::InvalidUnicodeEscape));
                }
            };
            value = (value << 4) | digit;
            self.offset += 1;
        }
        Ok(value)
    }

    /// Consumes one JSON number and returns its original lexical byte length.
    fn number_bytes(&mut self) -> Result<usize, JsonLexicalError<R, Q>> {
        let start = self.offset;
        if self.peek() == Some(b'-') {
            self.offset += 1;
        }
        match self.peek() {
            Some(b'0') => self.offset += 1,
            Some(b'1'..=b'9') => {
                self.offset += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.offset += 1;
                }
            }
            None => {
                return Err(self.syntax(JsonLexicalErrorReason::UnexpectedEnd));
            }
            Some(_) => {
                return Err(self.syntax(JsonLexicalErrorReason::InvalidNumber));
            }
        }
        if self.peek() == Some(b'.') {
            self.offset += 1;
            self.consume_digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            self.consume_digits()?;
        }
        if !Self::is_value_delimiter(self.peek()) {
            return Err(self.syntax(JsonLexicalErrorReason::InvalidNumber));
        }
        Ok(self.offset - start)
    }

    /// Validates one syntactically complete number against the supported
    /// integer and floating-point representation ranges.
    fn validate_number_range(&self, start: usize, end: usize) -> Result<(), JsonLexicalError<R, Q>> {
        let token = std::str::from_utf8(&self.input[start..end])
            .map_err(|_| self.syntax_at(start, JsonLexicalErrorReason::InvalidNumber))?;
        if token.contains(['.', 'e', 'E']) {
            let value = token
                .parse::<f64>()
                .map_err(|_| self.syntax_at(start, JsonLexicalErrorReason::FloatOutOfRange))?;
            if !value.is_finite() {
                return Err(self.syntax_at(start, JsonLexicalErrorReason::FloatOutOfRange));
            }
        } else if token.starts_with('-') {
            token
                .parse::<i64>()
                .map_err(|_| self.syntax_at(start, JsonLexicalErrorReason::IntegerOutOfRange))?;
        } else {
            token
                .parse::<u64>()
                .map_err(|_| self.syntax_at(start, JsonLexicalErrorReason::IntegerOutOfRange))?;
        }
        Ok(())
    }

    /// Consumes the required digits following a decimal point or exponent.
    fn consume_digits(&mut self) -> Result<(), JsonLexicalError<R, Q>> {
        if !matches!(self.peek(), Some(b'0'..=b'9')) {
            return Err(self.syntax(JsonLexicalErrorReason::InvalidNumber));
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.offset += 1;
        }
        Ok(())
    }

    /// Tests whether the next byte can follow a complete JSON scalar value.
    const fn is_value_delimiter(byte: Option<u8>) -> bool {
        matches!(byte, None | Some(b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}'))
    }

    /// Returns the UTF-8 width encoded by one valid leading byte.
    const fn utf8_width(byte: u8) -> Option<usize> {
        match byte {
            0xC2..=0xDF => Some(2),
            0xE0..=0xEF => Some(3),
            0xF0..=0xF4 => Some(4),
            _ => None,
        }
    }

    /// Computes one-based line and UTF-8 character column at the cursor.
    fn line_column(&self) -> (usize, usize) {
        self.line_column_at(self.offset)
    }

    /// Builds a structured syntax error at an explicit input byte offset.
    fn syntax_at(&self, offset: usize, reason: JsonLexicalErrorReason) -> JsonLexicalError<R, Q> {
        let (line, column) = self.line_column_at(offset);
        JsonLexicalError::Syntax(JsonLexicalFailure {
            offset,
            line,
            column,
            reason,
        })
    }

    /// Computes one-based line and UTF-8 character column at `offset`.
    fn line_column_at(&self, offset: usize) -> (usize, usize) {
        let end = offset.min(self.input.len());
        let mut line = 1;
        let mut column = 1;
        let mut index = 0;
        while index < end {
            match self.input[index] {
                b'\r' => {
                    if index + 1 < end && self.input[index + 1] == b'\n' {
                        index += 1;
                    }
                    line += 1;
                    column = 1;
                    index += 1;
                }
                b'\n' => {
                    line += 1;
                    column = 1;
                    index += 1;
                }
                _ => {
                    let character = std::str::from_utf8(&self.input[index..end])
                        .ok()
                        .and_then(|text| text.chars().next());
                    if let Some(character) = character {
                        index += character.len_utf8();
                    } else {
                        index += 1;
                    }
                    column += 1;
                }
            }
        }
        (line, column)
    }
}
