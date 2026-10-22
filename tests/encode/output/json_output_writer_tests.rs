// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests incremental output writing through the public encoder.

use std::collections::VecDeque;
use std::io;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::io::Write;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as _;
use serde::ser::SerializeSeq;

#[derive(Default)]
struct RecoveringWriter {
    bytes: Vec<u8>,
    calls: usize,
    flushes: usize,
}

impl Write for RecoveringWriter {
    fn write(&mut self, bytes: &[u8]) -> IoResult<usize> {
        self.calls += 1;
        if self.calls == 1 {
            return Err(ErrorKind::Interrupted.into());
        }
        let accepted = bytes.len().min(2);
        self.bytes.extend_from_slice(&bytes[..accepted]);
        Ok(accepted)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.flushes += 1;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum WriteAction {
    Accept(usize),
    Fail(io::ErrorKind),
}

struct ScriptedWriter {
    actions: VecDeque<WriteAction>,
    bytes: Vec<u8>,
    calls: usize,
    flushes: usize,
    flush_error: Option<io::ErrorKind>,
}

impl ScriptedWriter {
    fn new(actions: impl IntoIterator<Item = WriteAction>) -> Self {
        Self {
            actions: actions.into_iter().collect(),
            bytes: Vec::new(),
            calls: 0,
            flushes: 0,
            flush_error: None,
        }
    }
}

impl Write for ScriptedWriter {
    fn write(&mut self, bytes: &[u8]) -> IoResult<usize> {
        self.calls += 1;
        match self.actions.pop_front().unwrap_or(WriteAction::Accept(bytes.len())) {
            WriteAction::Accept(maximum) => {
                let accepted = bytes.len().min(maximum);
                self.bytes.extend_from_slice(&bytes[..accepted]);
                Ok(accepted)
            }
            WriteAction::Fail(kind) => Err(io::Error::new(kind, "scripted writer failure")),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        self.flushes += 1;
        match self.flush_error {
            Some(kind) => Err(io::Error::new(kind, "scripted flush failure")),
            None => Ok(()),
        }
    }
}

fn bounded_session() -> JsonEncodeSession<'static, JsonResource, usize> {
    JsonEncodeSession::from_limits(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_nodes(1)
            .max_output_bytes(16)
            .build(),
    )
}

struct MaskedWriteFailures;

impl Serialize for MaskedWriteFailures {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        let _ = sequence.serialize_element(&true);
        let _ = sequence.serialize_element(&false);
        Err(S::Error::custom("masked writer failures"))
    }
}

/// Verifies the incremental writer forwards accepted JSON bytes.
#[test]
fn test_json_output_writer_writes_incremental_document() {
    let session = JsonEncodeSession::from_limits(JsonEncodeLimits::<JsonResource, usize>::builder().build());
    let mut output = Vec::new();

    JsonEncoder::new(session)
        .write_incremental(&mut output, &true)
        .expect("incremental writer should accept a boolean");

    assert_eq!(output, b"true");
}

#[test]
fn test_json_writer_recovers_interrupted_write() {
    for buffered in [true, false] {
        for limited in [true, false] {
            let limits = if limited {
                JsonEncodeLimits::<JsonResource, usize>::builder()
                    .max_nodes(1)
                    .max_output_bytes(4)
                    .build()
            } else {
                JsonEncodeLimits::new()
            };
            let mut encoder = JsonEncoder::with_limits(limits);
            let mut output = RecoveringWriter::default();
            let result = if buffered {
                encoder.write_buffered(&mut output, &true)
            } else {
                encoder.write_incremental(&mut output, &true)
            };

            assert!(result.is_ok(), "buffered={buffered}, limited={limited}: {result:?}");
            assert_eq!(output.bytes, b"true");
            assert_eq!(output.calls, 3);
            assert_eq!(output.flushes, usize::from(!buffered));
            assert_eq!(encoder.session().value_budget().used_nodes(), limited.then_some(1));
            if limited {
                assert_eq!(encoder.session().output_budget().expect("output budget").used(), 4);
            }
        }
    }
}

#[cfg(feature = "jcs")]
#[test]
fn test_jcs_writer_recovers_interrupted_write() {
    use qubit_json::encode::JcsEncoder;
    use serde_json::json;

    for buffered in [true, false] {
        for limited in [true, false] {
            let limits = if limited {
                JsonEncodeLimits::<JsonResource, usize>::builder()
                    .max_nodes(1)
                    .max_output_bytes(4)
                    .build()
            } else {
                JsonEncodeLimits::new()
            };
            let mut encoder = JcsEncoder::with_limits(limits, 4096);
            let mut output = RecoveringWriter::default();
            let value = json!(true);
            let result = if buffered {
                encoder.write_buffered(&mut output, &value)
            } else {
                encoder.write_incremental(&mut output, &value)
            };

            assert!(result.is_ok(), "buffered={buffered}, limited={limited}: {result:?}");
            assert_eq!(output.bytes, b"true");
            assert_eq!(output.calls, 3);
            assert_eq!(output.flushes, usize::from(!buffered));
            assert_eq!(encoder.session().value_budget().used_nodes(), limited.then_some(1));
            if limited {
                assert_eq!(encoder.session().output_budget().expect("output budget").used(), 4);
            }
        }
    }
}

#[test]
fn test_json_writer_recovers_after_short_write_and_interrupted() {
    let mut encoder = JsonEncoder::new(bounded_session());
    let mut output = ScriptedWriter::new([
        WriteAction::Accept(2),
        WriteAction::Fail(io::ErrorKind::Interrupted),
        WriteAction::Accept(2),
    ]);

    encoder
        .write_incremental(&mut output, &true)
        .expect("Interrupted after a short write should be retried");

    assert_eq!(output.bytes, b"true");
    assert_eq!(output.calls, 3);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(encoder.session().output_budget().expect("output budget").used(), 4);
}

#[test]
fn test_json_writer_recovers_after_consecutive_interrupted_writes() {
    let mut encoder = JsonEncoder::new(bounded_session());
    let mut output = ScriptedWriter::new([
        WriteAction::Fail(io::ErrorKind::Interrupted),
        WriteAction::Fail(io::ErrorKind::Interrupted),
        WriteAction::Accept(4),
    ]);

    encoder
        .write_incremental(&mut output, &true)
        .expect("consecutive Interrupted errors should be retried");

    assert_eq!(output.bytes, b"true");
    assert_eq!(output.calls, 3);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(encoder.session().output_budget().expect("output budget").used(), 4);
}

#[test]
fn test_json_writer_preserves_interrupted_then_broken_pipe_failure() {
    let mut encoder = JsonEncoder::new(bounded_session());
    let mut output = ScriptedWriter::new([
        WriteAction::Accept(2),
        WriteAction::Fail(io::ErrorKind::Interrupted),
        WriteAction::Fail(io::ErrorKind::BrokenPipe),
    ]);

    let error = encoder
        .write_incremental(&mut output, &true)
        .expect_err("BrokenPipe must remain a terminal write error");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    assert_eq!(
        error.write_error().expect("write error").kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(output.bytes, b"tr");
    assert_eq!(output.calls, 3);
    assert_eq!(output.flushes, 0);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().expect("output budget").used(), 2);
}

#[test]
fn test_json_writer_preserves_write_zero_failure() {
    let mut encoder = JsonEncoder::new(bounded_session());
    let mut output = ScriptedWriter::new([WriteAction::Accept(0)]);

    let error = encoder
        .write_incremental(&mut output, &true)
        .expect_err("a zero-length write must fail");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    assert_eq!(
        error.write_error().expect("write error").kind(),
        io::ErrorKind::WriteZero
    );
    assert!(output.bytes.is_empty());
    assert_eq!(output.calls, 1);
    assert_eq!(output.flushes, 0);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().expect("output budget").used(), 0);
}

#[test]
fn test_json_incremental_writer_preserves_flush_interrupted_failure() {
    let mut encoder = JsonEncoder::new(bounded_session());
    let mut output = ScriptedWriter::new([WriteAction::Accept(4)]);
    output.flush_error = Some(io::ErrorKind::Interrupted);

    let error = encoder
        .write_incremental(&mut output, &true)
        .expect_err("flush Interrupted is terminal without a write_all retry");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    assert_eq!(
        error.write_error().expect("write error").kind(),
        io::ErrorKind::Interrupted
    );
    assert_eq!(output.bytes, b"true");
    assert_eq!(output.calls, 1);
    assert_eq!(output.flushes, 1);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().expect("output budget").used(), 4);
}

#[test]
fn test_json_writer_preserves_first_terminal_io_error() {
    let session = JsonEncodeSession::from_limits(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_nodes(4)
            .max_output_bytes(16)
            .build(),
    );
    let mut encoder = JsonEncoder::new(session);
    let mut output = ScriptedWriter::new([
        WriteAction::Accept(1),
        WriteAction::Fail(io::ErrorKind::BrokenPipe),
        WriteAction::Fail(io::ErrorKind::PermissionDenied),
    ]);

    let error = encoder
        .write_incremental(&mut output, &MaskedWriteFailures)
        .expect_err("the serializer masks the writer errors");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    assert_eq!(
        error.write_error().expect("write error").kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(output.bytes, b"[");
    assert_eq!(output.calls, 3);
    assert_eq!(output.flushes, 0);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().expect("output budget").used(), 1);
}
