// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared assertions for deterministic and fuzz JSON encoding invariants.

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::value::JsonValueEncoder;
use serde_json::Value;

use crate::input_limit::bounded_input;
use crate::json_encode_output::FuzzOutput;
use crate::json_encode_output::OutputMode;
use crate::json_number_contract::normalize_reference_value;
#[cfg(not(feature = "arbitrary-precision"))]
use crate::json_reference::parse_literal_value;

/// Number of control bytes preceding a JSON encode fuzz payload.
pub const CONTROL_BYTES: usize = 9;

type EncodeError = qubit_json::encode::JsonEncodeError<JsonResource, usize>;
type EncodeSession = JsonEncodeSession<'static, JsonResource, usize>;
type EncodeRun<T> = (Result<T, EncodeError>, EncodeSession);
type WriterRun = (Result<(), EncodeError>, Vec<u8>, EncodeSession);

/// Computes value nodes and payload bytes without using the JSON budget APIs.
fn reference_usage(value: &Value) -> (usize, usize) {
    let mut pending = vec![value];
    let mut nodes = 0_usize;
    let mut payload = 0_usize;
    while let Some(value) = pending.pop() {
        nodes = nodes.checked_add(1).expect("reference node count overflow");
        match value {
            Value::Null | Value::Bool(_) => {}
            Value::Number(number) => {
                payload = payload
                    .checked_add(
                        serde_json::to_string(number)
                            .expect("number serialization succeeds")
                            .len(),
                    )
                    .expect("reference payload count overflow");
            }
            Value::String(string) => {
                payload = payload
                    .checked_add(string.len())
                    .expect("reference payload count overflow");
            }
            Value::Array(values) => pending.extend(values.iter()),
            Value::Object(values) => {
                for (key, value) in values {
                    payload = payload
                        .checked_add(key.len())
                        .expect("reference payload count overflow");
                    pending.push(value);
                }
            }
        }
    }
    (nodes, payload)
}

/// Splits a bounded fuzz input into its independent control header and payload.
///
/// # Parameters
///
/// * `data` - Complete control header and JSON payload.
///
/// # Returns
///
/// Returns the fixed-size control header and remaining payload when the
/// complete input is within the shared fuzz limit and contains the header;
/// otherwise returns `None`.
pub fn split_encode_input(data: &[u8]) -> Option<(&[u8; CONTROL_BYTES], &[u8])> {
    let data = bounded_input(data)?;
    let (control, payload) = data.split_at_checked(CONTROL_BYTES)?;
    Some((control.try_into().expect("split header has fixed length"), payload))
}

/// Maps a two-byte selector to zero, a baseline boundary, or an arbitrary
/// limit.
///
/// # Parameters
///
/// * `raw` - Little-endian selector value from the control header.
/// * `baseline` - Explicitly measured resource use for successful encoding.
///
/// # Returns
///
/// The selected absolute resource limit.
pub fn select_encode_limit(raw: u16, baseline: usize) -> usize {
    match raw % 5 {
        0 => 0,
        1 => baseline.saturating_sub(1),
        2 => baseline,
        3 => baseline.saturating_add(1),
        _ => (usize::from(raw) / 5) % 4097,
    }
}

/// Checks output, value accounting, and injected writer invariants for an
/// input.
///
/// # Parameters
///
/// * `data` - Complete control header and candidate JSON payload.
///
/// # Panics
///
/// Panics when a valid JSON payload violates budget accounting, output
/// equivalence, rollback, or writer behavior invariants. Inputs without a
/// complete bounded header or valid JSON payload are ignored.
pub fn assert_json_encode_invariants(data: &[u8]) {
    let Some((control, payload)) = split_encode_input(data) else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<Value>(payload) else {
        return;
    };
    let normalized = match normalize_reference_value(&value) {
        Ok(normalized) => normalized,
        Err(expected_error) => {
            assert_unsupported_number_rejection(&value, expected_error);
            return;
        }
    };
    assert_transaction_projection_invariants(&value, &normalized);

    let mut baseline_encoder = JsonEncoder::new(encode_limits(usize::MAX, usize::MAX, usize::MAX));
    let baseline_encoded = baseline_encoder
        .to_vec(&value)
        .expect("a valid JSON Value must encode under effectively unbounded budgets");
    let baseline_session = baseline_encoder.into_session();
    let measured_nodes = baseline_session
        .value_budget()
        .used_nodes()
        .expect("baseline node measurement is enabled");
    let measured_payload = baseline_session
        .value_budget()
        .used_payload_bytes()
        .expect("baseline payload measurement is enabled");
    let measured_output = baseline_session
        .output_budget()
        .expect("baseline output measurement is enabled")
        .used();

    let (reference_nodes, reference_payload) = reference_usage(&value);
    let reference_encoded = serde_json::to_vec(&value).expect("JSON value serialization succeeds");
    let reference_output = reference_encoded.len();
    assert_eq!(
        measured_nodes, reference_nodes,
        "baseline node accounting must match independent reference"
    );
    assert_eq!(
        measured_payload, reference_payload,
        "baseline payload accounting must match independent reference"
    );
    assert_eq!(
        measured_output, reference_output,
        "baseline output accounting must match encoded byte length"
    );
    assert_eq!(baseline_encoded.as_slice(), reference_encoded.as_slice());
    let baseline_nodes = reference_nodes;
    let baseline_payload = reference_payload;
    let baseline_output = reference_output;

    let output_limit = select_control_limit(control, 0, baseline_output);
    let node_limit = select_control_limit(control, 2, baseline_nodes);
    let payload_limit = select_control_limit(control, 4, baseline_payload);
    let limits_fit =
        output_limit >= baseline_output && node_limit >= baseline_nodes && payload_limit >= baseline_payload;

    let (encoded, vector_session) = run_vector(&value, output_limit, node_limit, payload_limit);
    let (buffered, buffered_output, buffered_session) = run_buffered(&value, output_limit, node_limit, payload_limit);
    let (incremental, incremental_output, incremental_session) =
        run_incremental(&value, output_limit, node_limit, payload_limit);

    assert_accounting(
        &vector_session,
        (output_limit, node_limit, payload_limit),
        (baseline_output, baseline_nodes, baseline_payload),
        encoded.is_ok(),
    );
    assert_accounting(
        &buffered_session,
        (output_limit, node_limit, payload_limit),
        (baseline_output, baseline_nodes, baseline_payload),
        buffered.is_ok(),
    );
    assert_accounting(
        &incremental_session,
        (output_limit, node_limit, payload_limit),
        (baseline_output, baseline_nodes, baseline_payload),
        incremental.is_ok(),
    );
    if let Ok(encoded) = &encoded {
        assert_eq!(
            encoded.len(),
            vector_session
                .output_budget()
                .expect("output measurement is enabled")
                .used(),
            "to_vec byte length must match measured output accounting",
        );
    }
    assert_eq!(encoded.is_ok(), buffered.is_ok());
    assert_eq!(encoded.is_ok(), incremental.is_ok());
    assert_eq!(encoded.is_ok(), limits_fit);

    if let (Ok(encoded), Ok(()), Ok(())) = (encoded, buffered, incremental) {
        #[cfg(feature = "arbitrary-precision")]
        {
            let decoded: Value =
                serde_json::from_slice(&encoded).expect("successful budget-aware encoding must produce JSON");
            let decoded = normalize_reference_value(&decoded)
                .expect("encoded supported numbers normalize under the product contract");
            assert_eq!(decoded, normalized);
        }
        #[cfg(not(feature = "arbitrary-precision"))]
        {
            let decoded = parse_literal_value(&encoded).expect("successful budget-aware encoding must produce JSON");
            assert_eq!(decoded, normalized);
        }
        assert_eq!(encoded.as_slice(), baseline_encoded.as_slice());
        assert_eq!(buffered_output.as_slice(), baseline_encoded.as_slice());
        assert_eq!(incremental_output.as_slice(), baseline_encoded.as_slice());
    } else {
        assert_eq!(buffered_output.len(), 0, "buffered failure must publish no output");
    }

    assert_eq!(buffered_output.len(), buffered_session.output_budget().unwrap().used());
    assert_eq!(
        incremental_output.len(),
        incremental_session.output_budget().unwrap().used()
    );
    assert_writer_invariants(
        &value,
        &reference_encoded,
        control,
        baseline_nodes,
        baseline_payload,
        baseline_output,
    );
}

/// Checks explicit commit and rollback behavior for value projection.
pub fn assert_transaction_projection_invariants(value: &Value, normalized: &Value) {
    let (nodes, payload) = reference_usage(normalized);
    let encoder = JsonValueEncoder::new();
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(nodes)
        .max_payload_bytes(payload)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    {
        let mut transaction = budget.transaction();
        encoder
            .encode_in_transaction(value, &mut transaction)
            .expect("reference-sized limits admit the reference value");
        assert_eq!(transaction.used_nodes(), Some(nodes));
        assert_eq!(transaction.used_payload_bytes(), Some(payload));
        transaction.commit().expect("complete projection commits");
    }
    assert_eq!(budget.used_nodes(), Some(nodes));
    assert_eq!(budget.used_payload_bytes(), Some(payload));

    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(nodes.saturating_mul(2))
        .max_payload_bytes(payload.saturating_mul(2))
        .build();
    let mut batch_budget = JsonValueBudget::new(limits);
    {
        let mut transaction = batch_budget.transaction();
        for _ in 0..2 {
            encoder
                .encode_in_transaction(value, &mut transaction)
                .expect("both projections fit the combined transaction budget");
        }
        assert_eq!(transaction.used_nodes(), Some(nodes.saturating_mul(2)));
        assert_eq!(transaction.used_payload_bytes(), Some(payload.saturating_mul(2)));
        transaction.commit().expect("both projections commit together");
    }
    assert_eq!(batch_budget.used_nodes(), Some(nodes.saturating_mul(2)));
    assert_eq!(batch_budget.used_payload_bytes(), Some(payload.saturating_mul(2)));

    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(nodes.saturating_mul(2).saturating_sub(1))
        .max_payload_bytes(payload.saturating_mul(2))
        .build();
    let mut dropped_batch = JsonValueBudget::new(limits);
    {
        let mut transaction = dropped_batch.transaction();
        encoder
            .encode_in_transaction(value, &mut transaction)
            .expect("the first projection fits before the batch failure");
        assert!(encoder.encode_in_transaction(value, &mut transaction).is_err());
    }
    assert_eq!(dropped_batch.used_nodes(), Some(0));
    assert_eq!(dropped_batch.used_payload_bytes(), Some(0));
    {
        let mut transaction = dropped_batch.transaction();
        encoder
            .encode_in_transaction(value, &mut transaction)
            .expect("the rolled-back budget must accept a fresh transaction");
        transaction
            .commit()
            .expect("the fresh transaction commits after rollback");
    }
    assert_eq!(dropped_batch.used_nodes(), Some(nodes));
    assert_eq!(dropped_batch.used_payload_bytes(), Some(payload));

    let short_limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(nodes.saturating_sub(1))
        .max_payload_bytes(payload)
        .build();
    let mut short_budget = JsonValueBudget::new(short_limits);
    {
        let mut transaction = short_budget.transaction();
        let failed = encoder.encode_in_transaction(value, &mut transaction).is_err();
        assert_eq!(failed, nodes > 0, "one fewer node rejects a nonempty value");
    }
    assert_eq!(short_budget.used_nodes(), Some(0));
    assert_eq!(short_budget.used_payload_bytes(), Some(0));
}

fn assert_unsupported_number_rejection(value: &Value, expected: JsonSerializationErrorKind) {
    let projected = JsonValueEncoder::new().encode(value);
    assert_eq!(projected.unwrap_err().kind(), expected);

    let projection_limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(usize::MAX)
        .max_payload_bytes(usize::MAX)
        .build();
    let mut projection_budget = JsonValueBudget::new(projection_limits);
    {
        let mut transaction = projection_budget.transaction();
        let projected = JsonValueEncoder::new().encode_in_transaction(value, &mut transaction);
        assert_eq!(projected.unwrap_err().kind(), Some(expected));
    }
    assert_eq!(projection_budget.used_nodes(), Some(0));
    assert_eq!(projection_budget.used_payload_bytes(), Some(0));

    let mut encoder = JsonEncoder::new(encode_limits(usize::MAX, usize::MAX, usize::MAX));
    let encoded = encoder.to_vec(value);
    let error = encoded.unwrap_err();
    assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize);
    assert_eq!(error.serialization_error().unwrap().kind(), expected);
    let session = encoder.into_session();
    assert_eq!(session.output_budget().unwrap().used(), 0);
    assert_eq!(session.value_budget().used_nodes(), Some(0));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(0));

    let mut encoder = JsonEncoder::new(encode_limits(usize::MAX, usize::MAX, usize::MAX));
    let mut output = Vec::new();
    let encoded = encoder.write_buffered(&mut output, value);
    let error = encoded.unwrap_err();
    assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize);
    assert_eq!(error.serialization_error().unwrap().kind(), expected);
    assert!(output.is_empty(), "rejected number must publish no buffered output");
    let session = encoder.into_session();
    assert_eq!(session.value_budget().used_nodes(), Some(0));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(0));
}

fn select_control_limit(control: &[u8; CONTROL_BYTES], offset: usize, baseline: usize) -> usize {
    select_encode_limit(u16::from_le_bytes([control[offset], control[offset + 1]]), baseline)
}

fn encode_limits(output: usize, nodes: usize, payload: usize) -> EncodeSession {
    let structure = StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, nodes));
    let values = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(structure)
        .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, payload))
        .build();
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, output))
        .value_limits(values)
        .build();
    JsonEncodeSession::from_limits(limits)
}

fn run_vector(value: &Value, output: usize, nodes: usize, payload: usize) -> EncodeRun<Vec<u8>> {
    let mut encoder = JsonEncoder::new(encode_limits(output, nodes, payload));
    let result = encoder.to_vec(value);
    (result, encoder.into_session())
}

fn run_buffered(value: &Value, output_limit: usize, nodes: usize, payload: usize) -> WriterRun {
    let mut encoder = JsonEncoder::new(encode_limits(output_limit, nodes, payload));
    let mut output = Vec::new();
    let result = encoder.write_buffered(&mut output, value);
    (result, output, encoder.into_session())
}

fn run_incremental(value: &Value, output_limit: usize, nodes: usize, payload: usize) -> WriterRun {
    let mut encoder = JsonEncoder::new(encode_limits(output_limit, nodes, payload));
    let mut output = Vec::new();
    let result = encoder.write_incremental(&mut output, value);
    (result, output, encoder.into_session())
}

fn assert_accounting(
    session: &EncodeSession,
    limits: (usize, usize, usize),
    baseline: (usize, usize, usize),
    succeeded: bool,
) {
    let (output_limit, node_limit, payload_limit) = limits;
    let (baseline_output, baseline_nodes, baseline_payload) = baseline;
    let output_used = session.output_budget().expect("output measurement is enabled").used();
    let nodes_used = session
        .value_budget()
        .used_nodes()
        .expect("node measurement is enabled");
    let payload_used = session
        .value_budget()
        .used_payload_bytes()
        .expect("payload measurement is enabled");
    assert!(output_used <= output_limit);
    assert!(nodes_used <= node_limit);
    assert!(payload_used <= payload_limit);
    if succeeded {
        assert_eq!(output_used, baseline_output);
        assert_eq!(nodes_used, baseline_nodes);
        assert_eq!(payload_used, baseline_payload);
        return;
    }
    assert_eq!(nodes_used, 0, "failed encoding must roll back value nodes");
    assert_eq!(payload_used, 0, "failed encoding must roll back payload bytes");
}

fn assert_writer_invariants(
    value: &Value,
    expected: &[u8],
    control: &[u8; CONTROL_BYTES],
    baseline_nodes: usize,
    baseline_payload: usize,
    baseline_output: usize,
) {
    let mode = match control[6] % 3 {
        0 => OutputMode::Complete,
        1 => OutputMode::InterruptOnce,
        _ => OutputMode::BrokenPipe,
    };
    let accepted_limit = usize::from(control[7]);
    let max_chunk = 1 + usize::from(control[8] % 16);
    let remaining = if matches!(mode, OutputMode::BrokenPipe) {
        accepted_limit
    } else {
        usize::MAX
    };
    let mut output = FuzzOutput::new(mode, remaining, max_chunk);
    let mut encoder = JsonEncoder::new(encode_limits(usize::MAX, usize::MAX, usize::MAX));
    let result = encoder.write_incremental(&mut output, value);
    let session = encoder.into_session();
    let output_used = session.output_budget().expect("output measurement is enabled").used();
    assert_eq!(output_used, output.bytes().len());
    assert!(output_used <= baseline_output);
    assert_eq!(
        session.value_budget().used_nodes(),
        Some(if result.is_ok() { baseline_nodes } else { 0 })
    );
    assert_eq!(
        session.value_budget().used_payload_bytes(),
        Some(if result.is_ok() { baseline_payload } else { 0 })
    );
    if result.is_ok() {
        assert_eq!(output.bytes(), expected);
    } else {
        assert!(matches!(mode, OutputMode::BrokenPipe));
        assert_eq!(output.bytes(), &expected[..output.bytes().len()]);
        assert!(output.bytes().len() <= accepted_limit);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::reference_usage;

    #[test]
    fn test_reference_usage_counts_basic_values_and_numeric_tokens() {
        for literal in ["null", "true", "false", "\"\"", "[]", "{}"] {
            let value: Value = serde_json::from_str(literal).unwrap();
            assert_eq!(reference_usage(&value), (1, 0), "literal: {literal}");
        }

        let value: Value = serde_json::from_str(r#""é中""#).unwrap();
        assert_eq!(reference_usage(&value), (1, 5));
        let value: Value = serde_json::from_str(r#""a\n\"""#).unwrap();
        assert_eq!(reference_usage(&value), (1, 3));

        for (literal, expected_payload) in [
            ("18446744073709551615", 20),
            ("-9223372036854775808", 20),
            ("-0.0", 4),
            ("123.45", 6),
            ("1e+20", 5),
        ] {
            let value: Value = serde_json::from_str(literal).unwrap();
            assert_eq!(reference_usage(&value), (1, expected_payload), "number: {literal}");
        }
    }

    #[test]
    fn test_reference_usage_counts_utf8_object_keys_and_nested_values() {
        let value: Value = serde_json::from_str(r#"{"é":[null,true,"中",12]}"#).unwrap();
        assert_eq!(reference_usage(&value), (6, 7));

        let value: Value = serde_json::from_str(r#"{"escaped\"key":"a\n\""}"#).unwrap();
        assert_eq!(reference_usage(&value), (2, 14));
    }
}
