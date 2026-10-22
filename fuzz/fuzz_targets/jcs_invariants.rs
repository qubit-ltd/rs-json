// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes JCS byte compatibility and transactional resource accounting.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JcsDecoder;
use qubit_json::encode::JcsEncodeErrorKind;
use qubit_json::encode::JcsEncoder;
use qubit_json_fuzz::input_limit::bounded_input;
use qubit_json_fuzz::jcs_decode_invariants::assert_jcs_decode_invariants;
use serde_json::Value;

mod internal;

const MAX_DEPTH: usize = 32;

fuzz_target!(|data: &[u8]| {
    let Some(input) = bounded_input(data) else {
        return;
    };

    let raw_nodes = internal::fuzz_limit::limit(data, 6);
    assert_jcs_decode_invariants(input, raw_nodes, MAX_DEPTH);

    let Ok(value) = serde_json::from_slice::<Value>(input) else {
        return;
    };
    if !within_depth(&value) {
        return;
    }

    let expected = serde_json_canonicalizer::to_vec(&value)
        .expect("a serde_json value accepted by the JCS oracle must canonicalize");
    let mut unlimited = JcsEncoder::unlimited();
    assert_eq!(
        unlimited.to_vec(&value).expect("unlimited JCS encoding must succeed"),
        expected,
    );
    let decoded = JcsDecoder::unlimited()
        .decode_utf8(&expected)
        .expect("JCS encoder output must be decodable within the bounded depth");
    let recanonicalized = JcsEncoder::unlimited().to_vec(&decoded).unwrap();
    assert_eq!(recanonicalized, expected);

    let working_bytes = internal::fuzz_limit::limit(data, 0);
    let output_bytes = internal::fuzz_limit::limit(data, 2);
    let nodes = internal::fuzz_limit::limit(data, 4);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(output_bytes)
        .max_nodes(nodes)
        .build();
    let mut encoder = JcsEncoder::new(JsonEncodeSession::from_limits(limits), working_bytes);
    match encoder.to_vec(&value) {
        Ok(encoded) => {
            assert_eq!(encoded, expected);
            assert!(encoded.len() <= output_bytes);
            assert!(encoder.session().value_budget().used_nodes().unwrap_or_default() <= nodes);
            assert!(
                encoder
                    .session()
                    .output_budget()
                    .is_none_or(|budget| budget.used() <= output_bytes)
            );
        }
        Err(error) => {
            assert!(matches!(
                error.kind(),
                JcsEncodeErrorKind::Budget | JcsEncodeErrorKind::WorkingMemory | JcsEncodeErrorKind::Serialize
            ));
            assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
            assert!(
                encoder
                    .session()
                    .output_budget()
                    .is_none_or(|budget| budget.used() <= output_bytes)
            );
        }
    }
});

/// Returns whether every value lies within the configured structural depth.
fn within_depth(root: &Value) -> bool {
    let mut pending = vec![(root, 1_usize)];
    while let Some((value, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return false;
        }
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|child| (child, depth + 1))),
            Value::Object(entries) => pending.extend(entries.values().map(|child| (child, depth + 1))),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    true
}
