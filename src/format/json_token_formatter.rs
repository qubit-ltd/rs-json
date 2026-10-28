// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Syntax-only formatter that retains every source token verbatim.

use std::error::Error;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_io::ByteOutput;

use super::JsonPrettyError;
use super::JsonPrettyErrorKind;
use super::JsonPrettyOptions;
use super::internal::Action;
use super::internal::Plan;
use super::internal::Planner;
use crate::lexical::JsonLexicalScanError;
use crate::lexical::JsonLexicalScanner;
use crate::lexical::JsonNumberModel;

/// Validates and pretty-prints JSON while preserving source token spelling.
///
/// The complete input is scanned and the output plan is built before rendering.
/// Syntax, output-limit, and planning-capacity errors therefore produce no
/// output. Scalar token spellings are preserved verbatim, duplicate object
/// keys are accepted, numbers are checked for JSON syntax only, and no JCS
/// canonicalization is applied. Scanning continues through the full input so
/// syntax errors retain their existing precedence even after the output limit
/// has been exceeded.
///
/// # Parameters
///
/// * `input` - Bytes containing exactly one complete JSON value.
/// * `options` - Indentation and trailing-newline choices.
/// * `max_output_bytes` - Optional maximum formatted output length. `None`
///   disables this output limit; it does not limit input size, nesting depth,
///   or exact heap usage.
///
/// # Returns
///
/// The complete formatted byte vector.
///
/// # Errors
///
/// Returns `Syntax` for invalid or incomplete JSON, `OutputLimit` when the
/// planned bytes exceed the configured maximum, or `Capacity` when checked
/// length arithmetic or allocation fails. No result bytes are returned on
/// these errors.
///
/// # Examples
///
/// ```
/// use qubit_json::format::{format_json_tokens, JsonPrettyOptions};
///
/// let options = JsonPrettyOptions::builder().indent_width(2).build()?;
/// let output = format_json_tokens(b"[1]", &options, Some(16))?;
/// assert_eq!(output, b"[\n  1\n]");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn format_json_tokens(
    input: &[u8],
    options: &JsonPrettyOptions,
    max_output_bytes: Option<usize>,
) -> Result<Vec<u8>, JsonPrettyError> {
    let plan = make_plan(input, options, max_output_bytes)?;
    let mut output = Vec::new();
    output
        .try_reserve(plan.length)
        .map_err(|error| capacity_error(Some(Box::new(error))))?;
    render_vec(input, &plan.actions, &mut output);
    Ok(output)
}

/// Validates and plans the full document before writing to the supplied output.
///
/// The complete input is scanned before the first write. Syntax, output-limit,
/// and planning-capacity failures therefore leave the output untouched. Once
/// writing starts, an I/O error may leave a prefix accepted by the output; this
/// function does not make I/O atomic and does not flush the output. Scalar
/// token spellings are preserved, duplicate keys are accepted, numbers are
/// checked for syntax only, and JCS canonicalization is not performed. The
/// scanner consumes the complete input even after detecting that the planned
/// output exceeds its limit, preserving syntax-error precedence.
///
/// The output limit applies only to formatted byte length. It is not a bound on
/// input size, nesting depth, or exact heap usage. `None` disables the output
/// limit.
///
/// # Parameters
///
/// * `output` - Destination that receives formatted bytes; it is not flushed.
/// * `input` - Bytes containing exactly one complete JSON value.
/// * `options` - Indentation and trailing-newline choices.
/// * `max_output_bytes` - Optional maximum formatted output length.
///
/// # Returns
///
/// `Ok(())` after all planned bytes have been accepted by `output`.
///
/// # Errors
///
/// Returns `Syntax`, `OutputLimit`, or `Capacity` before writing begins.
/// Returns `Io` if a write fails; in that case the destination may already
/// contain an accepted prefix. No flush is attempted.
///
/// # Examples
///
/// ```
/// use qubit_json::format::{write_json_tokens, JsonPrettyOptions};
///
/// let options = JsonPrettyOptions::builder().indent_width(2).build()?;
/// let mut output = Vec::new();
/// write_json_tokens(&mut output, b"[1]", &options, Some(16))?;
/// assert_eq!(output, b"[\n  1\n]");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn write_json_tokens<O: ByteOutput + ?Sized>(
    output: &mut O,
    input: &[u8],
    options: &JsonPrettyOptions,
    max_output_bytes: Option<usize>,
) -> Result<(), JsonPrettyError> {
    let plan = make_plan(input, options, max_output_bytes)?;
    for action in &plan.actions {
        let bytes: &[u8] = match action {
            Action::Span(start, end) => &input[*start..*end],
            Action::Byte(byte) => std::slice::from_ref(byte),
            Action::Spaces(count) => {
                const SPACES: [u8; 256] = [b' '; 256];
                let mut remaining = *count;
                while remaining > 0 {
                    let chunk = remaining.min(SPACES.len());
                    output.write_fully(&SPACES[..chunk]).map_err(io_error)?;
                    remaining -= chunk;
                }
                continue;
            }
        };
        output.write_fully(bytes).map_err(io_error)?;
    }
    Ok(())
}

/// Scans a complete document and returns its output plan or a pre-write error.
///
/// Syntax and consumer errors stop planning immediately. If the output length
/// merely exceeds its limit, actions stop being retained but scanning
/// continues; after a successful scan this function reports `OutputLimit`.
/// Checked arithmetic, invalid planner state, or failed reservations can
/// instead report `Capacity`.
fn make_plan(input: &[u8], options: &JsonPrettyOptions, maximum: Option<usize>) -> Result<Plan, JsonPrettyError> {
    let mut planner = Planner {
        input,
        options,
        maximum,
        actions: Vec::new(),
        stack: Vec::new(),
        length: 0,
    };
    let limits = JsonDecodeLimits::<JsonResource, usize>::new();
    let mut session = JsonDecodeSession::from_limits(limits);
    let mut attempt = session.begin_value();
    let scan = {
        let mut scanner = JsonLexicalScanner::new(attempt.value_transaction_mut(), false)
            .with_number_model(JsonNumberModel::SyntaxOnly);
        scanner.scan_with_consumer(input, &mut planner)
    };
    if let Err(error) = scan {
        return Err(map_scan_error(error));
    }
    if !planner.stack.is_empty() {
        return Err(capacity_error(None));
    }
    if options.trailing_newline {
        planner.byte(b'\n')?;
    }
    if maximum.is_some_and(|maximum| planner.length > maximum) {
        return Err(JsonPrettyError::new(JsonPrettyErrorKind::OutputLimit, None, None));
    }
    Ok(Plan {
        actions: planner.actions,
        length: planner.length,
    })
}

/// Executes an already validated plan into a vector with reserved capacity.
///
/// The caller reserves the full planned length first; actions then copy source
/// spans or emit literal bytes and spaces without further fallible allocation.
fn render_vec(input: &[u8], actions: &[Action], output: &mut Vec<u8>) {
    const SPACES: [u8; 256] = [b' '; 256];
    for action in actions {
        match action {
            Action::Span(start, end) => output.extend_from_slice(&input[*start..*end]),
            Action::Byte(byte) => output.push(*byte),
            Action::Spaces(count) => {
                let mut remaining = *count;
                while remaining > 0 {
                    let chunk = remaining.min(SPACES.len());
                    output.extend_from_slice(&SPACES[..chunk]);
                    remaining -= chunk;
                }
            }
        }
    }
}

/// Maps scanner failures to the formatter's public error categories.
fn map_scan_error(error: JsonLexicalScanError<JsonPrettyError, JsonResource, usize>) -> JsonPrettyError {
    match error {
        JsonLexicalScanError::Lexical(crate::lexical::JsonLexicalError::Syntax(failure)) => JsonPrettyError::new(
            JsonPrettyErrorKind::Syntax,
            Some(failure.offset),
            Some(Box::new(failure)),
        ),
        JsonLexicalScanError::Lexical(_) => JsonPrettyError::new(JsonPrettyErrorKind::Syntax, None, None),
        JsonLexicalScanError::Consumer(error) => error,
        JsonLexicalScanError::EncodedStringLimit { offset, .. } => {
            JsonPrettyError::new(JsonPrettyErrorKind::Syntax, Some(offset), None)
        }
    }
}
/// Creates a `Capacity` error, retaining an allocation error as its source when
/// present.
pub(super) fn capacity_error(source: Option<Box<dyn Error + Send + Sync>>) -> JsonPrettyError {
    JsonPrettyError::new(JsonPrettyErrorKind::Capacity, None, source)
}
/// Wraps an output write failure as an `Io` error.
fn io_error(error: std::io::Error) -> JsonPrettyError {
    JsonPrettyError::new(JsonPrettyErrorKind::Io, None, Some(Box::new(error)))
}
