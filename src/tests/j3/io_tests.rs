// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::io;
use std::io::Write;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonResource;
use qubit_io::ByteOutput;

use crate::encode::JsonEncodeErrorKind;
use crate::encode::JsonEncoder;

#[derive(Clone, Copy)]
enum WriterMode {
    FailAfterThree,
    InterruptedOnce,
    WriteZero,
    FlushError,
}

struct ControlledWriter {
    bytes: Vec<u8>,
    accepted: usize,
    mode: WriterMode,
    interrupted: bool,
}

impl ControlledWriter {
    fn new(mode: WriterMode) -> Self {
        Self {
            bytes: Vec::new(),
            accepted: 0,
            mode,
            interrupted: false,
        }
    }
}

impl Write for ControlledWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        match self.mode {
            WriterMode::FailAfterThree if self.accepted >= 3 => {
                return Err(io::Error::other("controlled failure"));
            }
            WriterMode::InterruptedOnce if !self.interrupted => {
                self.interrupted = true;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            WriterMode::WriteZero => return Ok(0),
            _ => {}
        }

        let remaining = match self.mode {
            WriterMode::FailAfterThree => 3 - self.accepted,
            _ => usize::MAX,
        };
        let count = input.len().min(2).min(remaining);
        self.bytes.extend_from_slice(&input[..count]);
        self.accepted += count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        if matches!(self.mode, WriterMode::FlushError) {
            Err(io::Error::other("controlled flush failure"))
        } else {
            Ok(())
        }
    }
}

fn encoder_with_output_limit(limit: usize) -> JsonEncoder<'static> {
    JsonEncoder::with_limits(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_output_bytes(limit)
            .build(),
    )
}

#[test]
fn test_incremental_writer_charges_only_accepted_prefix() {
    let mut encoder = encoder_with_output_limit(64);
    let mut writer = ControlledWriter::new(WriterMode::FailAfterThree);
    let output: &mut dyn ByteOutput = &mut writer;
    let error = encoder.write_incremental(output, &"abcdef").unwrap_err();

    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    assert_eq!(writer.bytes, b"\"ab");
    assert_eq!(encoder.session().output_budget().unwrap().used(), 3);
}

#[test]
fn test_incremental_writer_retries_interrupted() {
    let mut encoder = JsonEncoder::unlimited();
    let mut writer = ControlledWriter::new(WriterMode::InterruptedOnce);
    let output: &mut dyn ByteOutput = &mut writer;
    encoder.write_incremental(output, &true).unwrap();

    assert_eq!(writer.bytes, b"true");
}

#[test]
fn test_incremental_writer_reports_write_zero() {
    let mut encoder = JsonEncoder::unlimited();
    let mut writer = ControlledWriter::new(WriterMode::WriteZero);
    let output: &mut dyn ByteOutput = &mut writer;
    let error = encoder.write_incremental(output, &true).unwrap_err();

    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    assert_eq!(error.write_error().unwrap().kind(), io::ErrorKind::WriteZero);
    assert!(writer.bytes.is_empty());
}

#[test]
fn test_incremental_writer_reports_flush_failure_and_charges_output() {
    let mut encoder = JsonEncoder::unlimited();
    let mut writer = ControlledWriter::new(WriterMode::FlushError);
    let output: &mut dyn ByteOutput = &mut writer;
    let error = encoder.write_incremental(output, &true).unwrap_err();

    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    assert_eq!(error.write_error().unwrap().kind(), io::ErrorKind::Other);
    assert_eq!(writer.bytes, b"true");
}
