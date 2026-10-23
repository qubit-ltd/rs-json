// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Iteratively renders JCS containers using bounded borrowed-reference tables.

use std::cmp::Ordering;
use std::io::Write;

use serde_json::Value;

use super::jcs_renderer_error::JcsRendererError;
use super::jcs_renderer_frame::JcsRendererFrame;
use super::jcs_reserve_error::JcsReserveError;
use super::jcs_working_memory::JcsWorkingMemory;

/// Renders JSON values iteratively without allocating a recursive traversal
/// stack.
pub(in crate::encode) struct JcsRenderer;

impl JcsRenderer {
    /// Writes the canonical representation of `value` to `writer`.
    ///
    /// The traversal stack and sorted object member tables are bounded by
    /// `max_working_bytes`. Serialization errors include failures from the
    /// writer as well as errors encoding JSON strings and numbers.
    ///
    /// # Parameters
    ///
    /// * `value` - JSON value whose canonical representation is rendered.
    /// * `writer` - Destination for the rendered bytes; write failures are
    ///   returned to the caller.
    /// * `max_working_bytes` - Maximum temporary memory reserved for traversal
    ///   frames and sorted object members.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after the complete value has been written.
    ///
    /// # Errors
    ///
    /// Returns an error if the writer fails, a string or number cannot be
    /// serialized, or the bounded working memory cannot be reserved.
    pub(in crate::encode) fn render<W: Write>(
        value: &Value,
        writer: &mut W,
        max_working_bytes: usize,
    ) -> Result<(), JcsRendererError> {
        let mut memory = JcsWorkingMemory::new(max_working_bytes);
        let mut stack = Vec::<JcsRendererFrame<'_>>::new();
        let mut current = Some(value);

        let result = (|| -> Result<(), JcsRendererError> {
            loop {
                if let Some(value) = current.take() {
                    match value {
                        Value::Array(values) => {
                            if let Some(first) = values.first() {
                                memory.reserve_stack(&mut stack, 1).map_err(map_reserve_error)?;
                                write(writer, b"[")?;
                                stack.push(JcsRendererFrame::Array { values, next: 1 });
                                current = Some(first);
                            } else {
                                write(writer, b"[")?;
                                write(writer, b"]")?;
                            }
                        }
                        Value::Object(object) => {
                            let mut entries = Vec::new();
                            memory
                                .reserve_object_entries(&mut entries, object.len())
                                .map_err(map_reserve_error)?;
                            entries.extend(object.iter().map(|(key, value)| (key.as_str(), value)));
                            entries.sort_unstable_by(|left, right| compare_utf16(left.0, right.0));
                            if entries.is_empty() {
                                write(writer, b"{")?;
                                write(writer, b"}")?;
                            } else {
                                memory.reserve_stack(&mut stack, 1).map_err(map_reserve_error)?;
                                write(writer, b"{")?;
                                let (key, child) = entries[0];
                                stack.push(JcsRendererFrame::Object { entries, next: 1 });
                                serde_json_canonicalizer::to_writer(&key, writer)
                                    .map_err(JcsRendererError::Serialize)?;
                                write(writer, b":")?;
                                current = Some(child);
                            }
                        }
                        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                            serde_json_canonicalizer::to_writer(value, writer).map_err(JcsRendererError::Serialize)?;
                        }
                    }
                    continue;
                }

                let Some(frame) = stack.last_mut() else {
                    return Ok(());
                };
                match frame {
                    JcsRendererFrame::Array { values, next } if *next < values.len() => {
                        if *next > 0 {
                            write(writer, b",")?;
                        }
                        current = Some(&values[*next]);
                        *next += 1;
                    }
                    JcsRendererFrame::Object { entries, next } if *next < entries.len() => {
                        if *next > 0 {
                            write(writer, b",")?;
                        }
                        let (key, child) = entries[*next];
                        *next += 1;
                        serde_json_canonicalizer::to_writer(&key, writer).map_err(JcsRendererError::Serialize)?;
                        write(writer, b":")?;
                        current = Some(child);
                    }
                    JcsRendererFrame::Array { .. } => {
                        stack.pop();
                        write(writer, b"]")?;
                    }
                    JcsRendererFrame::Object { .. } => {
                        if let Some(JcsRendererFrame::Object { entries, .. }) = stack.pop() {
                            memory.release_object_entries(&entries).map_err(map_reserve_error)?;
                            write(writer, b"}")?;
                        }
                    }
                }
            }
        })();
        #[cfg(feature = "jcs-allocation-diagnostics")]
        memory.report_diagnostics(result.is_ok());
        result
    }
}

/// Compares strings by the UTF-16 code units required by JCS object ordering.
///
/// # Parameters
///
/// * `left` - First string to compare.
/// * `right` - Second string to compare.
///
/// # Returns
///
/// Returns the ordering of `left` and `right` by their UTF-16 code units.
fn compare_utf16(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

/// Writes all bytes and converts I/O failures to the renderer's serialization
/// error.
///
/// # Parameters
///
/// * `writer` - Destination for the bytes.
/// * `bytes` - Bytes to write.
///
/// # Returns
///
/// Returns `Ok(())` when all bytes have been written.
///
/// # Errors
///
/// Returns an error if writing to `writer` fails.
fn write<W: Write>(writer: &mut W, bytes: &[u8]) -> Result<(), JcsRendererError> {
    writer
        .write_all(bytes)
        .map_err(|error| JcsRendererError::Serialize(serde_json::Error::io(error)))
}

/// Converts working-memory reservation failures to renderer errors.
///
/// # Parameters
///
/// * `error` - Working-memory reservation failure to convert.
///
/// # Returns
///
/// Returns the corresponding renderer error.
fn map_reserve_error(error: JcsReserveError) -> JcsRendererError {
    match error {
        JcsReserveError::Limit(error) => JcsRendererError::WorkingMemory(error),
        JcsReserveError::Allocation => JcsRendererError::Allocation,
    }
}
