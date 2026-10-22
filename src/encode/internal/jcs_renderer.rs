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

use super::jcs_working_memory::JcsReserveError;
use super::jcs_working_memory::JcsWorkingMemory;
use crate::encode::JcsWorkingMemoryError;

#[derive(Debug)]
pub(in crate::encode) enum JcsRendererError {
    WorkingMemory(JcsWorkingMemoryError),
    Serialize(serde_json::Error),
    Allocation,
}

enum Frame<'value> {
    Array {
        values: &'value [Value],
        next: usize,
    },
    Object {
        entries: Vec<(&'value str, &'value Value)>,
        next: usize,
    },
}

pub(in crate::encode) struct JcsRenderer;

impl JcsRenderer {
    pub(in crate::encode) fn render<W: Write>(
        value: &Value,
        writer: &mut W,
        max_working_bytes: usize,
    ) -> Result<(), JcsRendererError> {
        let mut memory = JcsWorkingMemory::new(max_working_bytes);
        let mut stack = Vec::<Frame<'_>>::new();
        let mut current = Some(value);

        loop {
            if let Some(value) = current.take() {
                match value {
                    Value::Array(values) => {
                        if let Some(first) = values.first() {
                            memory.reserve_stack(&mut stack, 1).map_err(map_reserve_error)?;
                            write(writer, b"[")?;
                            stack.push(Frame::Array { values, next: 1 });
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
                            stack.push(Frame::Object { entries, next: 1 });
                            serde_json_canonicalizer::to_writer(&key, writer).map_err(JcsRendererError::Serialize)?;
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
                Frame::Array { values, next } if *next < values.len() => {
                    if *next > 0 {
                        write(writer, b",")?;
                    }
                    current = Some(&values[*next]);
                    *next += 1;
                }
                Frame::Object { entries, next } if *next < entries.len() => {
                    if *next > 0 {
                        write(writer, b",")?;
                    }
                    let (key, child) = entries[*next];
                    *next += 1;
                    serde_json_canonicalizer::to_writer(&key, writer).map_err(JcsRendererError::Serialize)?;
                    write(writer, b":")?;
                    current = Some(child);
                }
                Frame::Array { .. } => {
                    stack.pop();
                    write(writer, b"]")?;
                }
                Frame::Object { .. } => {
                    if let Some(Frame::Object { entries, .. }) = stack.pop() {
                        memory.release_object_entries(&entries).map_err(map_reserve_error)?;
                        write(writer, b"}")?;
                    }
                }
            }
        }
    }
}

fn compare_utf16(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

fn write<W: Write>(writer: &mut W, bytes: &[u8]) -> Result<(), JcsRendererError> {
    writer
        .write_all(bytes)
        .map_err(|error| JcsRendererError::Serialize(serde_json::Error::io(error)))
}

fn map_reserve_error(error: JcsReserveError) -> JcsRendererError {
    match error {
        JcsReserveError::Limit(error) => JcsRendererError::WorkingMemory(error),
        JcsReserveError::Allocation => JcsRendererError::Allocation,
    }
}
