// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Non-recursive lexical admission for one JSON input document.
// qubit-style: allow source-test-pair

use qubit_budget::JsonValueBudget;
use qubit_budget::ResourceQuantity;

use super::super::JsonSerdeError;
use super::super::JsonSyntaxError;
use super::super::JsonSyntaxErrorReason;

/// Lexically validates and charges one JSON document without recursion.
pub(in crate::budget) struct JsonLexicalPreflight<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// JSON value resources charged while scanning the document.
    budget: &'a mut JsonValueBudget<R, Q>,

    /// Root-inclusive depth assigned to the inspected value.
    root_depth: usize,
}

impl<'a, R, Q> JsonLexicalPreflight<'a, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a lexical preflight bound to one mutable value budget.
    pub(in crate::budget) const fn new(
        budget: &'a mut JsonValueBudget<R, Q>,
    ) -> Self {
        Self {
            budget,
            root_depth: 1,
        }
    }

    /// Creates a lexical preflight rooted at an enclosing serializer depth.
    pub(in crate::budget) const fn at_depth(
        budget: &'a mut JsonValueBudget<R, Q>,
        root_depth: usize,
    ) -> Self {
        Self { budget, root_depth }
    }

    /// Validates and charges one complete JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`JsonSerdeError::Budget`] for the first resource violation, or
    /// [`JsonSerdeError::Json`] when `input` is not one complete JSON value.
    pub(in crate::budget) fn inspect(
        &mut self,
        input: &[u8],
    ) -> Result<(), JsonSerdeError<R, Q>> {
        let mut cursor = JsonCursor::new(input, self.budget);
        let mut stack = Vec::new();
        cursor.skip_whitespace();
        cursor.value(self.root_depth, &mut stack)?;
        while let Some(frame) = stack.pop() {
            cursor.resume(frame, &mut stack)?;
        }
        cursor.skip_whitespace();
        if cursor.position == input.len() {
            Ok(())
        } else {
            Err(cursor.syntax(JsonSyntaxErrorReason::TrailingCharacters))
        }
    }
}

/// Continuation for one JSON container being scanned iteratively.
enum ContainerFrame {
    /// An array ready for its first or next value.
    ArrayValue {
        /// Root-inclusive depth of the array.
        depth: usize,

        /// Items already admitted in this array.
        items: usize,
    },

    /// An array waiting for a comma or closing bracket.
    ArrayDelimiter {
        /// Root-inclusive depth of the array.
        depth: usize,

        /// Items already admitted in this array.
        items: usize,
    },

    /// An object ready for its first or next key.
    ObjectKey {
        /// Root-inclusive depth of the object.
        depth: usize,

        /// Entries already admitted in this object.
        entries: usize,
    },

    /// An object waiting for a comma or closing brace.
    ObjectDelimiter {
        /// Root-inclusive depth of the object.
        depth: usize,

        /// Entries already admitted in this object.
        entries: usize,
    },
}

/// Iterative cursor over the JSON bytes being admitted.
struct JsonCursor<'a, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Complete JSON input.
    input: &'a [u8],

    /// Current input position.
    position: usize,

    /// Value budget charged by lexical admission.
    budget: &'budget mut JsonValueBudget<R, Q>,
}

impl<'a, 'budget, R, Q> JsonCursor<'a, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a cursor positioned at the beginning of `input`.
    const fn new(
        input: &'a [u8],
        budget: &'budget mut JsonValueBudget<R, Q>,
    ) -> Self {
        Self {
            input,
            position: 0,
            budget,
        }
    }

    /// Advances past JSON whitespace.
    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.position += 1;
        }
    }

    /// Returns the current byte without advancing.
    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    /// Builds a structured syntax error at the current cursor position.
    fn syntax(&self, reason: JsonSyntaxErrorReason) -> JsonSerdeError<R, Q> {
        let (line, column) = line_column(self.input, self.position);
        JsonSyntaxError::new(self.position, line, column, reason).into()
    }

    /// Admits one JSON value and schedules any container continuation.
    fn value(
        &mut self,
        depth: usize,
        stack: &mut Vec<ContainerFrame>,
    ) -> Result<(), JsonSerdeError<R, Q>> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => {
                self.budget
                    .enter_node_usize(depth)
                    .map_err(JsonSerdeError::from)?;
                self.position += 1;
                stack.push(ContainerFrame::ObjectKey { depth, entries: 0 });
                Ok(())
            }
            Some(b'[') => {
                self.budget
                    .enter_node_usize(depth)
                    .map_err(JsonSerdeError::from)?;
                self.position += 1;
                stack.push(ContainerFrame::ArrayValue { depth, items: 0 });
                Ok(())
            }
            Some(b'"') => {
                self.budget
                    .enter_node_usize(depth)
                    .map_err(JsonSerdeError::from)?;
                let bytes = self.string_bytes()?;
                self.budget
                    .consume_string_bytes_usize(bytes)
                    .map_err(JsonSerdeError::from)
            }
            Some(b'-' | b'0'..=b'9') => {
                self.budget
                    .enter_node_usize(depth)
                    .map_err(JsonSerdeError::from)?;
                let bytes = self.number_bytes()?;
                self.budget
                    .consume_number_bytes_usize(bytes)
                    .map_err(JsonSerdeError::from)
            }
            Some(b't') => self.literal(depth, b"true"),
            Some(b'f') => self.literal(depth, b"false"),
            Some(b'n') => self.literal(depth, b"null"),
            None => Err(self.syntax(JsonSyntaxErrorReason::UnexpectedEnd)),
            Some(byte) => {
                Err(self.syntax(JsonSyntaxErrorReason::UnexpectedByte { byte }))
            }
        }
    }

    /// Charges and consumes one scalar literal.
    fn literal(
        &mut self,
        depth: usize,
        literal: &[u8],
    ) -> Result<(), JsonSerdeError<R, Q>> {
        if !self.input[self.position..].starts_with(literal) {
            return Err(match self.peek() {
                None => self.syntax(JsonSyntaxErrorReason::UnexpectedEnd),
                Some(byte) => {
                    self.syntax(JsonSyntaxErrorReason::UnexpectedByte { byte })
                }
            });
        }
        let end = self.position.saturating_add(literal.len());
        if !is_value_delimiter(self.input.get(end).copied()) {
            return Err(self.syntax(JsonSyntaxErrorReason::UnexpectedByte {
                byte: self.peek().unwrap_or_default(),
            }));
        }
        self.budget
            .enter_node_usize(depth)
            .map_err(JsonSerdeError::from)?;
        self.position = end;
        Ok(())
    }

    /// Resumes a container after its child value has completed.
    fn resume(
        &mut self,
        frame: ContainerFrame,
        stack: &mut Vec<ContainerFrame>,
    ) -> Result<(), JsonSerdeError<R, Q>> {
        match frame {
            ContainerFrame::ArrayValue { depth, items } => {
                self.skip_whitespace();
                if self.peek() == Some(b']') {
                    if items == 0 {
                        self.position += 1;
                        return Ok(());
                    }
                    return Err(self.syntax(
                        JsonSyntaxErrorReason::UnexpectedByte {
                            byte: self.peek().unwrap_or_default(),
                        },
                    ));
                }
                let items = items.checked_add(1).ok_or_else(|| {
                    self.syntax(JsonSyntaxErrorReason::NestingOverflow)
                })?;
                self.budget
                    .check_sequence_items_usize(items)
                    .map_err(JsonSerdeError::from)?;
                stack.push(ContainerFrame::ArrayDelimiter { depth, items });
                self.value(
                    depth.checked_add(1).ok_or_else(|| {
                        self.syntax(JsonSyntaxErrorReason::NestingOverflow)
                    })?,
                    stack,
                )
            }
            ContainerFrame::ArrayDelimiter { depth, items } => {
                self.skip_whitespace();
                match self.peek() {
                    Some(b',') => {
                        self.position += 1;
                        stack.push(ContainerFrame::ArrayValue { depth, items });
                        Ok(())
                    }
                    Some(b']') => {
                        self.position += 1;
                        Ok(())
                    }
                    None => {
                        Err(self.syntax(JsonSyntaxErrorReason::UnexpectedEnd))
                    }
                    Some(_) => Err(self.syntax(
                        JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
                    )),
                }
            }
            ContainerFrame::ObjectKey { depth, entries } => {
                self.skip_whitespace();
                if self.peek() == Some(b'}') {
                    if entries == 0 {
                        self.position += 1;
                        return Ok(());
                    }
                    return Err(self.syntax(
                        JsonSyntaxErrorReason::UnexpectedByte {
                            byte: self.peek().unwrap_or_default(),
                        },
                    ));
                }
                if self.peek() != Some(b'"') {
                    return Err(
                        self.syntax(JsonSyntaxErrorReason::ExpectedObjectKey)
                    );
                }
                let entries = entries.checked_add(1).ok_or_else(|| {
                    self.syntax(JsonSyntaxErrorReason::NestingOverflow)
                })?;
                self.budget
                    .check_map_entries_usize(entries)
                    .map_err(JsonSerdeError::from)?;
                let bytes = self.string_bytes()?;
                self.budget
                    .consume_key_bytes_usize(bytes)
                    .map_err(JsonSerdeError::from)?;
                self.skip_whitespace();
                if self.peek() != Some(b':') {
                    return Err(match self.peek() {
                        None => {
                            self.syntax(JsonSyntaxErrorReason::UnexpectedEnd)
                        }
                        Some(_) => {
                            self.syntax(JsonSyntaxErrorReason::ExpectedColon)
                        }
                    });
                }
                self.position += 1;
                stack.push(ContainerFrame::ObjectDelimiter { depth, entries });
                self.value(
                    depth.checked_add(1).ok_or_else(|| {
                        self.syntax(JsonSyntaxErrorReason::NestingOverflow)
                    })?,
                    stack,
                )
            }
            ContainerFrame::ObjectDelimiter { depth, entries } => {
                self.skip_whitespace();
                match self.peek() {
                    Some(b',') => {
                        self.position += 1;
                        stack
                            .push(ContainerFrame::ObjectKey { depth, entries });
                        Ok(())
                    }
                    Some(b'}') => {
                        self.position += 1;
                        Ok(())
                    }
                    None => {
                        Err(self.syntax(JsonSyntaxErrorReason::UnexpectedEnd))
                    }
                    Some(_) => Err(self.syntax(
                        JsonSyntaxErrorReason::ExpectedCommaOrObjectEnd,
                    )),
                }
            }
        }
    }

    /// Consumes one JSON string and returns its decoded UTF-8 byte length.
    fn string_bytes(&mut self) -> Result<usize, JsonSerdeError<R, Q>> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.position += 1;
        let mut decoded = 0_usize;
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.position += 1;
                    return Ok(decoded);
                }
                Some(b'\\') => {
                    self.position += 1;
                    let bytes = match self.peek() {
                        Some(
                            b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r'
                            | b't',
                        ) => {
                            self.position += 1;
                            1
                        }
                        Some(b'u') => self.unicode_escape_bytes()?,
                        None => {
                            return Err(self
                                .syntax(JsonSyntaxErrorReason::UnexpectedEnd));
                        }
                        Some(_) => {
                            return Err(self
                                .syntax(JsonSyntaxErrorReason::InvalidEscape));
                        }
                    };
                    decoded = decoded.checked_add(bytes).ok_or_else(|| {
                        self.syntax(JsonSyntaxErrorReason::NestingOverflow)
                    })?;
                }
                Some(0x20..=0x7F) => {
                    self.position += 1;
                    decoded = decoded.checked_add(1).ok_or_else(|| {
                        self.syntax(JsonSyntaxErrorReason::NestingOverflow)
                    })?;
                }
                Some(byte) if byte >= 0x80 => {
                    let width = utf8_width(byte).ok_or_else(|| {
                        self.syntax(JsonSyntaxErrorReason::InvalidUtf8)
                    })?;
                    let end =
                        self.position.checked_add(width).ok_or_else(|| {
                            self.syntax(JsonSyntaxErrorReason::NestingOverflow)
                        })?;
                    let text = self.input.get(self.position..end).ok_or_else(
                        || self.syntax(JsonSyntaxErrorReason::UnexpectedEnd),
                    )?;
                    let character = std::str::from_utf8(text)
                        .ok()
                        .and_then(|text| text.chars().next())
                        .filter(|character| character.len_utf8() == width)
                        .ok_or_else(|| {
                            self.syntax(JsonSyntaxErrorReason::InvalidUtf8)
                        })?;
                    self.position = end;
                    decoded = decoded
                        .checked_add(character.len_utf8())
                        .ok_or_else(|| {
                            self.syntax(JsonSyntaxErrorReason::NestingOverflow)
                        })?;
                }
                None => {
                    return Err(
                        self.syntax(JsonSyntaxErrorReason::UnexpectedEnd)
                    );
                }
                Some(byte) => {
                    return Err(self.syntax(
                        JsonSyntaxErrorReason::UnexpectedByte { byte },
                    ));
                }
            }
        }
    }

    /// Consumes a Unicode escape and returns its decoded UTF-8 byte length.
    fn unicode_escape_bytes(&mut self) -> Result<usize, JsonSerdeError<R, Q>> {
        debug_assert_eq!(self.peek(), Some(b'u'));
        self.position += 1;
        let first = self.hex_quad()?;
        let scalar = if (0xD800..=0xDBFF).contains(&first) {
            if self
                .input
                .get(self.position..self.position.saturating_add(2))
                != Some(b"\\u")
            {
                return Err(match self.peek() {
                    None => self.syntax(JsonSyntaxErrorReason::UnexpectedEnd),
                    Some(_) => {
                        self.syntax(JsonSyntaxErrorReason::UnpairedSurrogate)
                    }
                });
            }
            self.position += 2;
            let second = self.hex_quad()?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err(
                    self.syntax(JsonSyntaxErrorReason::UnpairedSurrogate)
                );
            }
            0x1_0000
                + ((u32::from(first) - 0xD800) << 10)
                + (u32::from(second) - 0xDC00)
        } else {
            if (0xDC00..=0xDFFF).contains(&first) {
                return Err(
                    self.syntax(JsonSyntaxErrorReason::UnpairedSurrogate)
                );
            }
            u32::from(first)
        };
        char::from_u32(scalar).map(char::len_utf8).ok_or_else(|| {
            self.syntax(JsonSyntaxErrorReason::UnpairedSurrogate)
        })
    }

    /// Consumes four hexadecimal digits from a Unicode escape.
    fn hex_quad(&mut self) -> Result<u16, JsonSerdeError<R, Q>> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let digit = match self.peek() {
                Some(byte @ b'0'..=b'9') => u16::from(byte - b'0'),
                Some(byte @ b'a'..=b'f') => u16::from(byte - b'a' + 10),
                Some(byte @ b'A'..=b'F') => u16::from(byte - b'A' + 10),
                None => {
                    return Err(
                        self.syntax(JsonSyntaxErrorReason::UnexpectedEnd)
                    );
                }
                Some(_) => {
                    return Err(self
                        .syntax(JsonSyntaxErrorReason::InvalidUnicodeEscape));
                }
            };
            value = (value << 4) | digit;
            self.position += 1;
        }
        Ok(value)
    }

    /// Consumes one JSON number and returns its original lexical byte length.
    fn number_bytes(&mut self) -> Result<usize, JsonSerdeError<R, Q>> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.position += 1;
        }
        match self.peek() {
            Some(b'0') => self.position += 1,
            Some(b'1'..=b'9') => {
                self.position += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.position += 1;
                }
            }
            None => {
                return Err(self.syntax(JsonSyntaxErrorReason::UnexpectedEnd));
            }
            Some(_) => {
                return Err(self.syntax(JsonSyntaxErrorReason::InvalidNumber));
            }
        }
        if self.peek() == Some(b'.') {
            self.position += 1;
            self.consume_digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            self.consume_digits()?;
        }
        if !is_value_delimiter(self.peek()) {
            return Err(self.syntax(JsonSyntaxErrorReason::InvalidNumber));
        }
        Ok(self.position - start)
    }

    /// Consumes the required digits following a decimal point or exponent.
    fn consume_digits(&mut self) -> Result<(), JsonSerdeError<R, Q>> {
        if !matches!(self.peek(), Some(b'0'..=b'9')) {
            return Err(self.syntax(JsonSyntaxErrorReason::InvalidNumber));
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.position += 1;
        }
        Ok(())
    }
}

/// Tests whether the next byte can follow a complete JSON scalar value.
const fn is_value_delimiter(byte: Option<u8>) -> bool {
    matches!(
        byte,
        None | Some(b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}')
    )
}

/// Returns the UTF-8 width encoded by one leading byte, when valid in width.
const fn utf8_width(byte: u8) -> Option<usize> {
    match byte {
        0xC2..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF4 => Some(4),
        _ => None,
    }
}

/// Computes one-based line and UTF-8 character column for a byte offset.
fn line_column(input: &[u8], offset: usize) -> (usize, usize) {
    let end = offset.min(input.len());
    let mut line = 1;
    let mut column = 1;
    let mut index = 0;
    while index < end {
        match input[index] {
            b'\r' => {
                if index + 1 < end && input[index + 1] == b'\n' {
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
                let character = std::str::from_utf8(&input[index..end])
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
