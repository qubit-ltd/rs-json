// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic structural JSON fixtures for unique-key decoder benchmarks.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

pub(crate) const COUNTS: [usize; 3] = [32, 512, 4096];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralShape {
    WideObject,
    RecordArray,
}

impl StructuralShape {
    pub(crate) const ALL: [Self; 2] = [Self::WideObject, Self::RecordArray];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::WideObject => "wide_object",
            Self::RecordArray => "record_array",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DuplicatePosition {
    Early,
    Late,
}

impl DuplicatePosition {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Early => "early",
            Self::Late => "late",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct StructuralRecord {
    pub(crate) id: u64,
    pub(crate) payload: String,
}

#[derive(Debug)]
pub(crate) enum StructuralExpected {
    Object(BTreeMap<String, String>),
    Records(Vec<StructuralRecord>),
}

#[derive(Debug)]
pub(crate) struct StructuralFixture {
    pub(crate) shape: StructuralShape,
    pub(crate) count: usize,
    pub(crate) text: String,
    pub(crate) expected: Option<StructuralExpected>,
    pub(crate) reject_position: Option<DuplicatePosition>,
    pub(crate) node_count: usize,
    pub(crate) payload_bytes: usize,
    pub(crate) max_depth: usize,
    pub(crate) max_sequence_items: usize,
    pub(crate) max_map_entries: usize,
    pub(crate) max_key_bytes: usize,
    pub(crate) max_string_bytes: usize,
    pub(crate) max_number_bytes: usize,
}

fn object_values(count: usize) -> BTreeMap<String, String> {
    (0..count).map(|i| (format!("k{i:06}"), format!("v{i:06}"))).collect()
}

fn record_values(count: usize) -> Vec<StructuralRecord> {
    (0..count)
        .map(|i| StructuralRecord {
            id: 7,
            payload: format!("v{i:06}"),
        })
        .collect()
}

fn assert_count(count: usize) {
    assert!(COUNTS.contains(&count), "structural count must be one of COUNTS");
}

pub(crate) fn success_fixture(shape: StructuralShape, count: usize) -> StructuralFixture {
    assert_count(count);
    let (text, expected, node_count, payload_bytes, max_depth, max_sequence_items, max_map_entries, max_number_bytes) =
        match shape {
            StructuralShape::WideObject => {
                let values = object_values(count);
                let text = serde_json::to_string(&values).expect("object fixture must serialize");
                let parsed: serde_json::Value = serde_json::from_str(&text).expect("object fixture must parse");
                assert_eq!(parsed.as_object().expect("object fixture is object").len(), count);
                for (index, (key, value)) in values.iter().enumerate() {
                    assert_eq!(parsed[key], *value);
                    assert_eq!(key, &format!("k{index:06}"));
                }
                (
                    text,
                    Some(StructuralExpected::Object(values)),
                    count + 1,
                    14 * count,
                    2,
                    0,
                    count,
                    0,
                )
            }
            StructuralShape::RecordArray => {
                let values = record_values(count);
                let text = serde_json::to_string(&values).expect("record fixture must serialize");
                let parsed: serde_json::Value = serde_json::from_str(&text).expect("record fixture must parse");
                assert_eq!(parsed.as_array().expect("record fixture is array").len(), count);
                for (index, record) in values.iter().enumerate() {
                    assert_eq!(parsed[index]["id"], 7);
                    assert_eq!(parsed[index]["payload"], record.payload);
                }
                (
                    text,
                    Some(StructuralExpected::Records(values)),
                    3 * count + 1,
                    17 * count,
                    3,
                    count,
                    2,
                    1,
                )
            }
        };
    let fixture = StructuralFixture {
        shape,
        count,
        text,
        expected,
        reject_position: None,
        node_count,
        payload_bytes,
        max_depth,
        max_sequence_items,
        max_map_entries,
        max_key_bytes: 7,
        max_string_bytes: 7,
        max_number_bytes,
    };
    let expected_bytes = match shape {
        StructuralShape::WideObject => 19 * count + 1,
        StructuralShape::RecordArray => 29 * count + 1,
    };
    assert_eq!(fixture.text.len(), expected_bytes, "fixture byte budget formula");
    fixture
}

pub(crate) fn reject_fixture(count: usize, position: DuplicatePosition) -> StructuralFixture {
    assert_count(count);
    let values = object_values(count);
    let mut members = values
        .iter()
        .map(|(key, value)| {
            format!(
                "{}:{}",
                serde_json::to_string(key).unwrap(),
                serde_json::to_string(value).unwrap()
            )
        })
        .collect::<Vec<_>>();
    let duplicate = r#""k000000":"duplicate""#;
    let text = match position {
        DuplicatePosition::Early => {
            members.insert(1, duplicate.to_owned());
            format!("{{{}}}", members.join(","))
        }
        DuplicatePosition::Late => {
            members.push(duplicate.to_owned());
            format!("{{{}}}", members.join(","))
        }
    };
    let fixture = StructuralFixture {
        shape: StructuralShape::WideObject,
        count,
        text,
        expected: None,
        reject_position: Some(position),
        node_count: count + 2,
        payload_bytes: 14 * count + 16,
        max_depth: 2,
        max_sequence_items: 0,
        max_map_entries: count + 1,
        max_key_bytes: 7,
        max_string_bytes: 9,
        max_number_bytes: 0,
    };
    assert_eq!(
        fixture.text.len(),
        19 * count + 22,
        "duplicate fixture byte budget formula"
    );
    assert_eq!(fixture.text.matches("\"k000000\"").count(), 2);
    fixture
}
