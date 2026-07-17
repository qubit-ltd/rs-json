// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#![no_main]

mod internal;

use libfuzzer_sys::fuzz_target;
use qubit_json::{
    JsonDecodeErrorKind,
    JsonDecodeOptions,
    LenientJsonDecoder,
    MarkdownFenceClosing,
    MarkdownFencePolicy,
};

use internal::FuzzRecord;

fuzz_target!(|data: &[u8]| {
    let default_decoder = LenientJsonDecoder::default();
    if let Ok(value) =
        default_decoder.decode_slice::<serde_json::Value>(data)
    {
        let encoded = serde_json::to_vec(&value)
            .expect("serde_json::Value must serialize");
        let _: serde_json::Value = serde_json::from_slice(&encoded)
            .expect("successful decoder output must be strict JSON");
    }
    if !data.is_empty() {
        let bounded = LenientJsonDecoder::new(
            JsonDecodeOptions::strict()
                .with_max_input_bytes(Some(data.len() - 1)),
        );
        let error = bounded
            .decode_slice::<serde_json::Value>(data)
            .expect_err("an input above its raw byte limit must fail");
        assert_eq!(error.kind(), JsonDecodeErrorKind::InputTooLarge);
    }

    let strict_result = LenientJsonDecoder::new(JsonDecodeOptions::strict())
        .decode_slice::<serde_json::Value>(data);
    let serde_result = serde_json::from_slice::<serde_json::Value>(data);
    match (strict_result, serde_result) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
        (Err(_), Err(_)) => {}
        (Ok(_), Err(_)) => {
            panic!("strict decoder accepted input rejected by serde_json");
        }
        (Err(_), Ok(_)) => {
            panic!("strict decoder rejected input accepted by serde_json");
        }
    }

    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    let decoders = [
        default_decoder.clone(),
        LenientJsonDecoder::new(JsonDecodeOptions::strict()),
        LenientJsonDecoder::new(
            JsonDecodeOptions::lenient().with_markdown_fence_policy(
                MarkdownFencePolicy::Any {
                    closing: MarkdownFenceClosing::Optional,
                },
            ),
        ),
        LenientJsonDecoder::new(
            JsonDecodeOptions::lenient().with_markdown_fence_policy(
                MarkdownFencePolicy::JsonOnly {
                    closing: MarkdownFenceClosing::Required,
                },
            ),
        ),
    ];

    for decoder in decoders {
        let _ = decoder.decode::<FuzzRecord>(input);
        let _ = decoder.decode_object::<FuzzRecord>(input);
        let _ = decoder.decode_array::<FuzzRecord>(input);
        let _ = decoder.decode_value(input);
    }

    if let Ok(value) =
        default_decoder.decode_object::<serde_json::Value>(input)
    {
        assert!(value.is_object());
    }
    if let Ok(values) =
        default_decoder.decode_array::<serde_json::Value>(input)
    {
        let encoded = serde_json::to_vec(&values)
            .expect("decoded array elements must serialize");
        let reparsed: serde_json::Value = serde_json::from_slice(&encoded)
            .expect("decoded array must remain strict JSON");
        assert!(reparsed.is_array());
    }
    if input.contains("TOP_SECRET")
        && let Err(error) = default_decoder.decode::<FuzzRecord>(input)
    {
        assert!(!error.message().contains("TOP_SECRET"));
        assert!(!error.to_string().contains("TOP_SECRET"));
        assert!(!format!("{error:?}").contains("TOP_SECRET"));
    }
});
