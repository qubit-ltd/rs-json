// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests incremental output writing through the public encoder.

use std::collections::VecDeque;
use std::fmt;
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

/// Writer that can fail once during writing or flushing to preserve an error
/// source.
struct SourceWriter {
    write_error: Option<io::Error>,
    flush_error: Option<io::Error>,
}

impl Write for SourceWriter {
    fn write(&mut self, bytes: &[u8]) -> IoResult<usize> {
        match self.write_error.take() {
            Some(error) => Err(error),
            None => Ok(bytes.len()),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        match self.flush_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

/// Distinct source error used to verify that wrapped I/O errors retain their
/// cause.
#[derive(Debug)]
struct MarkerError;

impl fmt::Display for MarkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("marker source failure")
    }
}

impl std::error::Error for MarkerError {}

/// Selects which original I/O error shape the encoder must preserve.
#[derive(Clone, Copy)]
enum PreservedError {
    Os,
    Marker,
}

impl PreservedError {
    fn make(self) -> io::Error {
        match self {
            Self::Os => io::Error::from_raw_os_error(5),
            Self::Marker => io::Error::other(MarkerError),
        }
    }

    fn assert_preserved(self, error: &io::Error) {
        match self {
            Self::Os => {
                assert_eq!(error.raw_os_error(), Some(5));
                assert_eq!(error.kind(), io::Error::from_raw_os_error(5).kind());
            }
            Self::Marker => assert!(
                error
                    .get_ref()
                    .and_then(|source| source.downcast_ref::<MarkerError>())
                    .is_some()
            ),
        }
    }
}

/// Selects the encoder output operation that receives an injected failure.
#[derive(Clone, Copy)]
enum OutputFailure {
    BufferedWrite,
    IncrementalWrite,
    IncrementalFlush,
}

/// Creates a writer that injects the selected error at the selected operation.
fn source_writer(failure: OutputFailure, error: PreservedError) -> SourceWriter {
    match failure {
        OutputFailure::BufferedWrite | OutputFailure::IncrementalWrite => SourceWriter {
            write_error: Some(error.make()),
            flush_error: None,
        },
        OutputFailure::IncrementalFlush => SourceWriter {
            write_error: None,
            flush_error: Some(error.make()),
        },
    }
}

/// Checks that JSON encoding reports the write error without losing its source.
fn assert_json_error_preserves_original(failure: OutputFailure, original: PreservedError) {
    let mut encoder = JsonEncoder::new(bounded_session());
    let result = match failure {
        OutputFailure::BufferedWrite => encoder.write_buffered(source_writer(failure, original), &true),
        OutputFailure::IncrementalWrite | OutputFailure::IncrementalFlush => {
            encoder.write_incremental(source_writer(failure, original), &true)
        }
    };
    let error = result.expect_err("the configured output operation should fail");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    original.assert_preserved(error.write_error().expect("write error"));
}

/// Checks that JCS encoding reports the write error without losing its source.
#[cfg(feature = "jcs")]
fn assert_jcs_error_preserves_original(failure: OutputFailure, original: PreservedError) {
    use qubit_json::encode::JcsEncoder;
    use serde_json::json;

    let mut encoder = JcsEncoder::new(bounded_session(), 4096);
    let value = json!(true);
    let result = match failure {
        OutputFailure::BufferedWrite => encoder.write_buffered(source_writer(failure, original), &value),
        OutputFailure::IncrementalWrite | OutputFailure::IncrementalFlush => {
            encoder.write_incremental(source_writer(failure, original), &value)
        }
    };
    let error = result.expect_err("the configured output operation should fail");
    original.assert_preserved(error.write_error().expect("write error"));
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

/// Serializer that replaces element-write errors with its own serialization
/// error.
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

#[test]
fn test_json_output_writer_preserves_original_write_errors() {
    for failure in [OutputFailure::BufferedWrite, OutputFailure::IncrementalWrite] {
        for original in [PreservedError::Os, PreservedError::Marker] {
            assert_json_error_preserves_original(failure, original);
        }
    }
}

#[test]
fn test_json_output_writer_preserves_original_incremental_flush_errors() {
    for original in [PreservedError::Os, PreservedError::Marker] {
        assert_json_error_preserves_original(OutputFailure::IncrementalFlush, original);
    }
}

#[cfg(feature = "jcs")]
#[test]
fn test_jcs_output_writer_preserves_original_write_errors() {
    for failure in [OutputFailure::BufferedWrite, OutputFailure::IncrementalWrite] {
        for original in [PreservedError::Os, PreservedError::Marker] {
            assert_jcs_error_preserves_original(failure, original);
        }
    }
}

#[cfg(feature = "jcs")]
#[test]
fn test_jcs_output_writer_preserves_original_incremental_flush_errors() {
    for original in [PreservedError::Os, PreservedError::Marker] {
        assert_jcs_error_preserves_original(OutputFailure::IncrementalFlush, original);
    }
}
