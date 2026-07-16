// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_json::{
    JsonDecodeOptions,
    LenientJsonDecoder,
    MarkdownFenceClosing,
    MarkdownFencePolicy,
};
use serde::Deserialize;

/// Minimal typed payload used to exercise typed decoder entry points.
#[derive(Deserialize)]
struct FuzzRecord;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    let decoders = [
        LenientJsonDecoder::default(),
        LenientJsonDecoder::new(JsonDecodeOptions::strict()),
        LenientJsonDecoder::new(JsonDecodeOptions::json_code_fences_only()),
        LenientJsonDecoder::new(
            JsonDecodeOptions::json_code_fences_only()
                .with_markdown_fence_policy(
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
});
