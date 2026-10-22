// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Assertions shared by deterministic tests and the JCS decode fuzz target.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JcsDecodeError;
use qubit_json::decode::JcsDecoder;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::encode::JcsEncoder;
use serde_json::Value;

/// Checks JCS decoding against an unlimited-node baseline and configured node
/// and depth limits.
///
/// Both decoders admit the complete input under the same byte and depth
/// constraints and measure value nodes. A successful baseline supplies the
/// expected node count so this harness does not need to parse JSON itself.
///
/// # Parameters
///
/// * `input` - Candidate complete JSON document.
/// * `max_nodes` - Maximum number of value nodes accepted by the constrained
///   decoder.
/// * `max_depth` - Maximum structural depth accepted by both decoders.
///
/// # Panics
///
/// Panics if input accounting, value accounting, rejection rollback, or
/// canonical encode/decode invariants do not match the configured limits.
pub fn assert_jcs_decode_invariants(input: &[u8], max_nodes: usize, max_depth: usize) {
    let baseline_limits = decode_limits(input, usize::MAX, max_depth);
    let mut baseline = JcsDecoder::with_limits(baseline_limits);
    let baseline_result = baseline.decode_utf8(input);
    let baseline_nodes = baseline.session().value_budget().used_nodes();
    assert!(baseline_nodes.is_some(), "baseline must measure nodes");
    assert_eq!(
        baseline.session().input_budget().map(|budget| budget.used()),
        Some(input.len()),
        "baseline must account for the complete raw input",
    );

    let constrained_limits = decode_limits(input, max_nodes, max_depth);
    assert_eq!(
        constrained_limits.value_limits().max_nodes(),
        Some(max_nodes),
        "the shared limit builder must preserve max_nodes",
    );
    let mut constrained = JcsDecoder::with_limits(constrained_limits);
    let constrained_result = constrained.decode_utf8(input);
    assert_eq!(
        constrained.session().input_budget().map(|budget| budget.used()),
        Some(input.len()),
        "constrained decoder must account for the complete raw input",
    );

    match baseline_result {
        Ok(expected) => {
            let expected_nodes = baseline_nodes.expect("node measurement is enabled");
            if max_nodes < expected_nodes {
                let error =
                    constrained_result.expect_err("a node limit below the baseline count must reject the value");
                assert_decode_budget_error(error);
                assert_eq!(constrained.session().value_budget().used_nodes(), Some(0));
            } else {
                let actual =
                    constrained_result.expect("a node limit that fits the baseline count must accept the value");
                assert_eq!(actual, expected);
                assert_eq!(constrained.session().value_budget().used_nodes(), Some(expected_nodes));
                assert_canonical_round_trip(&actual);
            }
        }
        Err(_) => {
            assert!(
                constrained_result.is_err(),
                "a document rejected by the baseline must not be accepted under stricter node limits",
            );
            assert_eq!(constrained.session().value_budget().used_nodes(), Some(0));
        }
    }
}

fn decode_limits(input: &[u8], max_nodes: usize, max_depth: usize) -> JsonDecodeLimits<JsonResource, usize> {
    JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(input.len())
        .max_depth(max_depth)
        .max_nodes(max_nodes)
        .build()
}

fn assert_decode_budget_error(error: JcsDecodeError) {
    let JcsDecodeError::Decode(error) = error else {
        panic!("a node limit must be reported as a decode budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

fn assert_canonical_round_trip(value: &Value) {
    let encoded = JcsEncoder::unlimited()
        .to_vec(value)
        .expect("accepted value must encode as JCS");
    let reparsed = JcsDecoder::unlimited()
        .decode_utf8(&encoded)
        .expect("canonical JCS output must decode");
    let reencoded = JcsEncoder::unlimited()
        .to_vec(&reparsed)
        .expect("reparsed value must encode as JCS");
    assert_eq!(reencoded, encoded);
}
