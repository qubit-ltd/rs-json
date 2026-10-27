// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::format::JsonPrettyErrorKind;
use crate::format::JsonPrettyOptions;
use crate::format::format_json_tokens;
use crate::format::write_json_tokens;

#[test]
fn test_pretty_preserves_duplicate_keys_and_original_tokens() {
    let options = JsonPrettyOptions::builder().build().unwrap();
    let output = format_json_tokens(br#"{"x":1e400,"x":1.00,"z":-0,"s":"\u0061"}"#, &options, None).unwrap();
    assert_eq!(
        output,
        b"{\n  \"x\": 1e400,\n  \"x\": 1.00,\n  \"z\": -0,\n  \"s\": \"\\u0061\"\n}",
    );
}

#[test]
fn test_pretty_error_kinds_have_stable_messages() {
    assert_eq!(JsonPrettyErrorKind::Syntax.to_string(), "invalid JSON syntax");
    assert_eq!(
        JsonPrettyErrorKind::OutputLimit.to_string(),
        "formatted output limit exceeded"
    );
    assert_eq!(
        JsonPrettyErrorKind::Capacity.to_string(),
        "formatting capacity exceeded"
    );
    assert_eq!(JsonPrettyErrorKind::Io.to_string(), "formatted output write failed");
}

#[test]
fn test_pretty_rejects_before_writing_any_bytes() {
    let options = JsonPrettyOptions::builder().build().unwrap();
    let mut output = Vec::new();
    assert!(write_json_tokens(&mut output, b"[1,]", &options, None).is_err());
    assert!(output.is_empty());
    assert!(write_json_tokens(&mut output, b"[1]", &options, Some(1)).is_err());
    assert!(output.is_empty());
}

#[test]
fn test_pretty_empty_containers_limits_and_final_newline() {
    let options = JsonPrettyOptions::builder().build().unwrap();
    assert_eq!(format_json_tokens(b"{ }", &options, None).unwrap(), b"{}");
    assert_eq!(format_json_tokens(b"[ ]", &options, None).unwrap(), b"[]");
    let newline = JsonPrettyOptions::builder().trailing_newline(true).build().unwrap();
    assert_eq!(format_json_tokens(b" true \n", &newline, None).unwrap(), b"true\n");
    let no_spaces = JsonPrettyOptions::builder().indent_width(0).build().unwrap();
    assert_eq!(
        format_json_tokens(br#"{"x":1}"#, &no_spaces, None).unwrap(),
        b"{\n\"x\": 1\n}"
    );
}

#[test]
fn test_pretty_preserves_false_null_and_nested_container_layout() {
    let options = JsonPrettyOptions::builder().build().unwrap();
    let output = format_json_tokens(br#"{"items":[false,null,{"nested":"\u0061"}]}"#, &options, None).unwrap();

    assert_eq!(
        output,
        b"{\n  \"items\": [\n    false,\n    null,\n    {\n      \"nested\": \"\\u0061\"\n    }\n  ]\n}"
    );
}

#[test]
fn test_pretty_truncation_and_invalid_tokens_are_syntax_errors() {
    let options = JsonPrettyOptions::builder().build().unwrap();
    for input in [b"[1,]".as_slice(), b"[1", b"[\"\\q\"]", b"NaN", b"{}x"] {
        let mut output = Vec::new();
        let error = write_json_tokens(&mut output, input, &options, None).unwrap_err();
        assert_eq!(error.kind(), JsonPrettyErrorKind::Syntax);
        assert!(!format!("{error:?}").is_empty());
        assert!(!error.to_string().is_empty());
        assert!(output.is_empty());
        if input == b"{}x" {
            assert_eq!(error.offset(), Some(2));
            assert!(std::error::Error::source(&error).is_some());
        }
    }
}

#[test]
fn test_pretty_output_limit_exact_boundary_and_checked_indent_overflow() {
    let options = JsonPrettyOptions::builder().build().unwrap();
    let input = br#"{"a":[1,2]}"#;
    let formatted = format_json_tokens(input, &options, None).unwrap();
    assert_eq!(
        format_json_tokens(input, &options, Some(formatted.len())).unwrap(),
        formatted
    );
    let newline = JsonPrettyOptions::builder().trailing_newline(true).build().unwrap();
    let with_newline = format_json_tokens(input, &newline, None).unwrap();
    assert_eq!(with_newline.last(), Some(&b'\n'));
    assert_eq!(
        format_json_tokens(input, &newline, Some(with_newline.len())).unwrap(),
        with_newline
    );
    let error = format_json_tokens(input, &newline, Some(with_newline.len() - 1)).unwrap_err();
    assert_eq!(error.kind(), JsonPrettyErrorKind::OutputLimit);
    let mut output = Vec::new();
    let error = write_json_tokens(&mut output, input, &options, Some(formatted.len() - 1)).unwrap_err();
    assert_eq!(error.kind(), JsonPrettyErrorKind::OutputLimit);
    assert!(output.is_empty());
    assert_eq!(format_json_tokens(b"{}", &options, Some(2)).unwrap(), b"{}");
    assert_eq!(
        format_json_tokens(b"0", &options, Some(0)).unwrap_err().kind(),
        JsonPrettyErrorKind::OutputLimit
    );
    let wide = JsonPrettyOptions::builder().indent_width(usize::MAX).build().unwrap();
    let error = format_json_tokens(b"[[]]", &wide, None).unwrap_err();
    assert_eq!(error.kind(), JsonPrettyErrorKind::Capacity);
}

#[test]
fn test_pretty_depth_saturation_retains_real_level() {
    let mut input = vec![b'['; 64];
    input.push(b'0');
    input.extend(std::iter::repeat_n(b']', 64));
    let options = JsonPrettyOptions::builder().max_indent_levels(Some(3)).build().unwrap();
    let output = format_json_tokens(&input, &options, None).unwrap();
    assert!(output.windows(7).any(|window| window == b"      0"));
    assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 128);

    let no_levels = JsonPrettyOptions::builder()
        .indent_width(2)
        .max_indent_levels(Some(0))
        .build()
        .unwrap();
    assert_eq!(format_json_tokens(b"[0]", &no_levels, None).unwrap(), b"[\n0\n]");

    let mut deep = vec![b'['; 32_768];
    deep.push(b'0');
    deep.extend(std::iter::repeat_n(b']', 32_768));
    let output = format_json_tokens(&deep, &options, None).unwrap();
    assert!(output.starts_with(b"[\n  [\n    [\n      [\n      ["));
    assert!(output.ends_with(b"\n      ]\n    ]\n  ]\n]"));
}

#[test]
fn test_pretty_large_indentation_writes_all_space_chunks() {
    let options = JsonPrettyOptions::builder().indent_width(300).build().unwrap();
    let mut expected = b"[\n".to_vec();
    expected.extend(std::iter::repeat_n(b' ', 300));
    expected.extend_from_slice(b"0\n]");

    assert_eq!(format_json_tokens(b"[0]", &options, None).unwrap(), expected);

    let mut output = Vec::new();
    write_json_tokens(&mut output, b"[0]", &options, None).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn test_pretty_partial_writer_keeps_accepted_prefix_and_dyn_output_works() {
    use std::cell::RefCell;
    use std::io;
    use std::io::Write;
    use std::rc::Rc;

    use qubit_io::ByteOutput;

    struct FailAfterThree(Rc<RefCell<Vec<u8>>>);
    impl Write for FailAfterThree {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let remaining = 3usize.saturating_sub(self.0.borrow().len());
            if remaining == 0 {
                return Err(io::Error::other("controlled failure"));
            }
            let count = remaining.min(bytes.len());
            self.0.borrow_mut().extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let options = JsonPrettyOptions::builder().build().unwrap();
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let mut output: Box<dyn ByteOutput> = Box::new(FailAfterThree(Rc::clone(&bytes)));
    let error = write_json_tokens(output.as_mut(), b"[1,2]", &options, None).unwrap_err();
    assert_eq!(error.kind(), JsonPrettyErrorKind::Io);
    assert_eq!(&*bytes.borrow(), b"[\n ");
}

#[test]
fn test_pretty_writer_retries_interrupted_and_reports_write_zero() {
    use std::io;
    use std::io::Write;

    struct ControlledWriter {
        bytes: Vec<u8>,
        interrupt_once: bool,
        write_zero: bool,
    }

    impl Write for ControlledWriter {
        fn write(&mut self, input: &[u8]) -> io::Result<usize> {
            if self.interrupt_once {
                self.interrupt_once = false;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            if self.write_zero {
                return Ok(0);
            }
            self.bytes.extend_from_slice(input);
            Ok(input.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let options = JsonPrettyOptions::builder().build().unwrap();
    let mut interrupted = ControlledWriter {
        bytes: Vec::new(),
        interrupt_once: true,
        write_zero: false,
    };
    write_json_tokens(&mut interrupted, b"[1]", &options, None).unwrap();
    assert_eq!(interrupted.bytes, b"[\n  1\n]");

    let mut zero = ControlledWriter {
        bytes: Vec::new(),
        interrupt_once: false,
        write_zero: true,
    };
    let error = write_json_tokens(&mut zero, b"true", &options, None).unwrap_err();
    assert_eq!(error.kind(), JsonPrettyErrorKind::Io);
    assert_eq!(
        std::error::Error::source(&error)
            .and_then(|source| source.downcast_ref::<io::Error>())
            .map(io::Error::kind),
        Some(io::ErrorKind::WriteZero)
    );
}
