// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::format::JsonPrettyError;
use crate::format::JsonPrettyOptions;
use crate::format::internal::Action;
use crate::format::internal::Container;
use crate::format::internal::ContainerKind;
use crate::lexical::JsonLexicalConsumer;
use crate::lexical::JsonLexicalEvent;

/// Accumulates output operations while the scanner validates the whole input.
///
/// # Type Parameters
///
/// * `'a` - Lifetime of the input bytes and formatting options borrowed during
///   planning.
pub(in crate::format) struct Planner<'a> {
    /// Source bytes referenced by planned span actions.
    pub(in crate::format) input: &'a [u8],
    /// Formatting choices used to plan indentation.
    pub(in crate::format) options: &'a JsonPrettyOptions,
    /// Optional maximum retained output length; exceeding it does not stop
    /// scanning.
    pub(in crate::format) maximum: Option<usize>,
    /// Operations retained for rendering, including only the prefix within the
    /// limit.
    pub(in crate::format) actions: Vec<Action>,
    /// Open container formatting state, ordered from outermost to innermost.
    pub(in crate::format) stack: Vec<Container>,
    /// Full planned output length, including bytes beyond `maximum`.
    pub(in crate::format) length: usize,
}

impl Planner<'_> {
    /// Plans one literal byte.
    ///
    /// # Parameters
    ///
    /// * `byte` - Literal output byte to retain if it falls within the limit.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after accounting for the byte.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if length accounting or action allocation fails.
    #[inline]
    pub(in crate::format) fn byte(&mut self, byte: u8) -> Result<(), JsonPrettyError> {
        self.push(Action::Byte(byte), 1)
    }

    /// Adds to the full output length, returning `Capacity` if it cannot be
    /// represented.
    ///
    /// # Parameters
    ///
    /// * `amount` - Number of output bytes to add to the full planned length.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after updating the full planned length.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if the accumulated length overflows.
    fn add_len(&mut self, amount: usize) -> Result<(), JsonPrettyError> {
        self.length = self
            .length
            .checked_add(amount)
            .ok_or_else(|| crate::format::json_token_formatter::capacity_error(None))?;
        Ok(())
    }
    /// Accumulates the full planned length even after the output limit is
    /// exceeded. Actions past the limit are not retained, but scanning
    /// continues so a later syntax error is reported before the final
    /// output-limit rejection.
    /// Records an operation unless the full length has crossed the configured
    /// limit.
    ///
    /// # Parameters
    ///
    /// * `action` - A validated source span or literal operation.
    /// * `length` - Number of output bytes represented by the operation.
    ///
    /// # Returns
    ///
    /// `Ok(())` after accounting for the full length; an over-limit operation
    /// is simply not retained.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if length arithmetic or action allocation fails.
    fn push(&mut self, action: Action, length: usize) -> Result<(), JsonPrettyError> {
        self.add_len(length)?;
        if self.maximum.is_some_and(|maximum| self.length > maximum) {
            return Ok(());
        }
        self.actions
            .try_reserve(1)
            .map_err(|error| crate::format::json_token_formatter::capacity_error(Some(Box::new(error))))?;
        self.actions.push(action);
        Ok(())
    }
    /// Plans a checked source range; invalid or overflowing ranges are capacity
    /// errors.
    ///
    /// # Parameters
    ///
    /// * `offset` - Zero-based byte offset into the original input.
    /// * `length` - Number of source bytes to include.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after planning the checked source range.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if the range overflows, extends past the input, or
    /// cannot be accounted for.
    fn span(&mut self, offset: usize, length: usize) -> Result<(), JsonPrettyError> {
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= self.input.len())
            .ok_or_else(|| crate::format::json_token_formatter::capacity_error(None))?;
        self.push(Action::Span(offset, end), length)
    }
    /// Plans indentation for the requested nesting level, subject to configured
    /// caps.
    ///
    /// # Parameters
    ///
    /// * `levels` - Current nesting level used to compute the indentation.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after planning the indentation bytes.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if indentation arithmetic or action allocation fails.
    fn spaces(&mut self, levels: usize) -> Result<(), JsonPrettyError> {
        let shown = self
            .options
            .max_indent_levels
            .map_or(levels, |maximum| levels.min(maximum));
        let count = self
            .options
            .indent_width
            .checked_mul(shown)
            .ok_or_else(|| crate::format::json_token_formatter::capacity_error(None))?;
        if count != 0 {
            self.push(Action::Spaces(count), count)?;
        }
        Ok(())
    }
    /// Plans punctuation and indentation before the next value in the current
    /// container.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after accounting for the next value's prefix.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if the container state is invalid or output planning
    /// cannot be represented or retained.
    fn prepare_value(&mut self) -> Result<(), JsonPrettyError> {
        let Some(frame) = self.stack.last_mut() else {
            return Ok(());
        };
        match frame.kind {
            ContainerKind::Array => {
                let first = frame.count == 0;
                frame.count = frame
                    .count
                    .checked_add(1)
                    .ok_or_else(|| crate::format::json_token_formatter::capacity_error(None))?;
                if !first {
                    self.byte(b',')?;
                }
                self.byte(b'\n')?;
                self.spaces(self.stack.len())
            }
            ContainerKind::Object => {
                if !frame.awaiting_value {
                    return Err(crate::format::json_token_formatter::capacity_error(None));
                }
                frame.awaiting_value = false;
                self.byte(b':')?;
                self.byte(b' ')
            }
        }
    }
    /// Plans an object key and updates the frame to expect its value.
    ///
    /// # Parameters
    ///
    /// * `offset` - Zero-based byte offset of the key in the original input.
    /// * `length` - Number of source bytes occupied by the key token.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after planning the key and marking its value pending.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if there is no object frame, its state is invalid, or
    /// output planning cannot be represented or retained.
    fn key(&mut self, offset: usize, length: usize) -> Result<(), JsonPrettyError> {
        let Some(frame) = self.stack.last_mut() else {
            return Err(crate::format::json_token_formatter::capacity_error(None));
        };
        if !matches!(frame.kind, ContainerKind::Object) || frame.awaiting_value {
            return Err(crate::format::json_token_formatter::capacity_error(None));
        }
        let first = frame.count == 0;
        frame.count = frame
            .count
            .checked_add(1)
            .ok_or_else(|| crate::format::json_token_formatter::capacity_error(None))?;
        frame.awaiting_value = true;
        if !first {
            self.byte(b',')?;
        }
        self.byte(b'\n')?;
        self.spaces(self.stack.len())?;
        self.span(offset, length)
    }
    /// Plans an opening delimiter and pushes a new container frame.
    ///
    /// # Parameters
    ///
    /// * `kind` - Container kind associated with the opening delimiter.
    /// * `offset` - Zero-based byte offset of the delimiter in the input.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after planning the delimiter and opening a frame.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if the delimiter range is invalid or planner storage
    /// cannot be reserved.
    fn begin(&mut self, kind: ContainerKind, offset: usize) -> Result<(), JsonPrettyError> {
        self.span(offset, 1)?;
        self.stack
            .try_reserve(1)
            .map_err(|error| crate::format::json_token_formatter::capacity_error(Some(Box::new(error))))?;
        self.stack.push(Container {
            kind,
            count: 0,
            awaiting_value: false,
        });
        Ok(())
    }
    /// Validates and plans a closing delimiter, popping the matching frame.
    ///
    /// # Parameters
    ///
    /// * `offset` - Zero-based byte offset of the closing delimiter.
    /// * `expected` - Container kind required to match the open frame.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after planning the delimiter and closing its frame.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if no matching frame exists or the closing state is
    /// invalid.
    fn end(&mut self, offset: usize, expected: ContainerKind) -> Result<(), JsonPrettyError> {
        let frame = self
            .stack
            .pop()
            .ok_or_else(|| crate::format::json_token_formatter::capacity_error(None))?;
        if !matches!(
            (frame.kind, expected),
            (ContainerKind::Array, ContainerKind::Array) | (ContainerKind::Object, ContainerKind::Object)
        ) || frame.awaiting_value
        {
            return Err(crate::format::json_token_formatter::capacity_error(None));
        }
        if frame.count > 0 {
            self.byte(b'\n')?;
            self.spaces(self.stack.len())?;
        }
        self.span(offset, 1)?;
        Ok(())
    }
}

impl<'input> JsonLexicalConsumer<'input> for Planner<'_> {
    /// Error used to abort scanning when planner state or length is invalid.
    type Error = JsonPrettyError;

    /// Prepares the current container before the scanner processes a value.
    ///
    /// # Parameters
    ///
    /// * `_depth` - Root-inclusive nesting depth supplied by the scanner.
    /// * `_offset` - Zero-based byte offset where the value begins.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the planner accepts the current container state.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if the current container state or resulting output
    /// accounting is invalid.
    fn before_value(&mut self, _depth: usize, _offset: usize) -> Result<(), Self::Error> {
        self.prepare_value()
    }

    /// Accepts the scanner's pre-key callback; key formatting occurs when its
    /// corresponding key event is received.
    ///
    /// # Parameters
    ///
    /// * `_offset` - Zero-based byte offset where the key begins.
    ///
    /// # Returns
    ///
    /// Always returns `Ok(())`; this callback defers key processing to `event`.
    #[inline]
    fn before_key(&mut self, _offset: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Accepts the pre-number callback; the number span is planned on its
    /// event.
    ///
    /// # Parameters
    ///
    /// * `_token` - Number spelling borrowed from the scanned input.
    /// * `_offset` - Zero-based byte offset where the number begins.
    ///
    /// # Returns
    ///
    /// Always returns `Ok(())`; this callback defers span planning to `event`.
    #[inline]
    fn before_number(&mut self, _token: &'input str, _offset: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Converts each scanner event into the corresponding output operation.
    ///
    /// # Parameters
    ///
    /// * `event` - Recognized token or container delimiter with its input
    ///   offset.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after planning the operation represented by the event.
    ///
    /// # Errors
    ///
    /// Returns `Capacity` if event state is invalid or its output operation
    /// cannot be represented or retained.
    fn event(&mut self, event: JsonLexicalEvent<'input>) -> Result<(), Self::Error> {
        match event {
            JsonLexicalEvent::BeginArray { offset } => self.begin(ContainerKind::Array, offset),
            JsonLexicalEvent::BeginObject { offset } => self.begin(ContainerKind::Object, offset),
            JsonLexicalEvent::EndArray { offset } => self.end(offset, ContainerKind::Array),
            JsonLexicalEvent::EndObject { offset } => self.end(offset, ContainerKind::Object),
            JsonLexicalEvent::Key { token, offset } => self.key(offset, token.len()),
            JsonLexicalEvent::String { token, offset } | JsonLexicalEvent::Number { token, offset } => {
                self.span(offset, token.len())
            }
            JsonLexicalEvent::Null { offset } => self.span(offset, 4),
            JsonLexicalEvent::Boolean { value, offset } => self.span(offset, if value { 4 } else { 5 }),
        }
    }
}

#[cfg(test)]
mod tests {
    use qubit_budget::json::JsonDecodeLimits;
    use qubit_budget::json::JsonDecodeSession;
    use qubit_budget::json::JsonResource;

    use super::Planner;
    use crate::format::JsonPrettyErrorKind;
    use crate::format::JsonPrettyOptions;
    use crate::format::internal::Container;
    use crate::format::internal::ContainerKind;
    use crate::lexical::JsonLexicalScanner;
    use crate::lexical::JsonNumberModel;

    #[test]
    fn test_planner_rejects_invalid_spans_and_unmatched_end() {
        let options = JsonPrettyOptions::builder().build().expect("valid defaults");
        let mut planner = Planner {
            input: b"[]",
            options: &options,
            maximum: None,
            actions: Vec::new(),
            stack: Vec::new(),
            length: 0,
        };
        for (offset, length) in [(usize::MAX, 2), (1, 2)] {
            assert_eq!(
                planner.span(offset, length).expect_err("invalid span").kind(),
                JsonPrettyErrorKind::Capacity
            );
            assert!(planner.actions.is_empty());
            assert_eq!(planner.length, 0);
        }
        assert_eq!(
            planner.end(1, ContainerKind::Array).expect_err("missing frame").kind(),
            JsonPrettyErrorKind::Capacity
        );
    }

    #[test]
    fn test_planner_container_counts_do_not_wrap() {
        let options = JsonPrettyOptions::builder().build().expect("valid defaults");
        for kind in [ContainerKind::Array, ContainerKind::Object] {
            let mut planner = Planner {
                input: b"[]",
                options: &options,
                maximum: None,
                actions: Vec::new(),
                length: 0,
                stack: vec![Container {
                    kind,
                    count: usize::MAX,
                    awaiting_value: false,
                }],
            };
            let result = match kind {
                ContainerKind::Array => planner.prepare_value(),
                ContainerKind::Object => planner.key(0, 1),
            };
            assert_eq!(
                result.expect_err("item count must not wrap").kind(),
                JsonPrettyErrorKind::Capacity
            );
            assert_eq!(planner.stack[0].count, usize::MAX);
            assert!(planner.actions.is_empty());
        }
    }

    #[test]
    fn test_planner_stops_retaining_actions_after_output_limit() {
        fn planned_storage(input: &[u8], options: &JsonPrettyOptions) -> (usize, usize) {
            let mut planner = Planner {
                input,
                options,
                maximum: Some(8),
                actions: Vec::new(),
                stack: Vec::new(),
                length: 0,
            };
            let limits = JsonDecodeLimits::<JsonResource, usize>::new();
            let mut session = JsonDecodeSession::from_limits(limits);
            let mut attempt = session.begin_value();
            let mut scanner = JsonLexicalScanner::new(attempt.value_transaction_mut(), false)
                .with_number_model(JsonNumberModel::SyntaxOnly);
            scanner
                .scan_with_consumer(input, &mut planner)
                .expect("valid array must scan");
            (planner.actions.len(), planner.actions.capacity())
        }

        let options = JsonPrettyOptions::builder().build().expect("valid defaults");
        let mut zero_limit_planner = Planner {
            input: b"0",
            options: &options,
            maximum: Some(0),
            actions: Vec::new(),
            stack: Vec::new(),
            length: 0,
        };
        zero_limit_planner
            .push(crate::format::internal::Action::Byte(b'0'), 1)
            .expect("crossing the limit does not stop scanning");
        assert_eq!(zero_limit_planner.actions.len(), 0, "zero limit retains no actions");
        assert_eq!(
            zero_limit_planner.actions.capacity(),
            0,
            "zero limit allocates no action storage"
        );

        let small = format!("[{}]", vec!["0"; 32].join(","));
        let large = format!("[{}]", vec!["0"; 4_096].join(","));
        let small_storage = planned_storage(small.as_bytes(), &options);
        let large_storage = planned_storage(large.as_bytes(), &options);
        assert_eq!(
            small_storage.0, large_storage.0,
            "retained actions must stop at the same prefix"
        );
        assert_eq!(
            small_storage.1, large_storage.1,
            "action capacity must not grow after the limit"
        );
        assert!(
            large_storage.0 > 0,
            "the planner retains the prefix before exceeding the limit"
        );
    }
}
