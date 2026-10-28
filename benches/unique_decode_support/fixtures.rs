// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic, owned JSON documents used by both benchmark binaries.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    Config,
    Metadata,
    WireEnvelope,
}

impl Shape {
    pub(crate) const ALL: [Self; 3] = [Self::Config, Self::Metadata, Self::WireEnvelope];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Metadata => "metadata",
            Self::WireEnvelope => "wire_envelope",
        }
    }
}

#[derive(Debug)]
pub(crate) struct Fixture {
    pub(crate) shape: Shape,
    pub(crate) text: String,
    pub(crate) target_bytes: usize,
    pub(crate) node_count: usize,
    pub(crate) expected: Option<Expected>,
    pub(crate) max_depth: usize,
    pub(crate) max_sequence_items: usize,
    pub(crate) max_map_entries: usize,
    pub(crate) max_key_bytes: usize,
    pub(crate) max_string_bytes: usize,
    pub(crate) max_number_bytes: usize,
    pub(crate) payload_bytes: usize,
}

#[derive(Debug)]
pub(crate) enum Expected {
    Config(Config),
    Metadata(Metadata),
    WireEnvelope(WireEnvelope),
}

pub(crate) fn reject_fixture(shape: Shape, target_bytes: usize) -> Fixture {
    let filler = "x".repeat(target_bytes.saturating_sub(80).max(1));
    let text = match shape {
        Shape::Config | Shape::Metadata => {
            format!(r#"{{"name":"first","name":"second","payload":"{filler}"}}"#)
        }
        Shape::WireEnvelope => {
            format!(r#"{{"version":1,"version":2,"records":[{{"id":7,"payload":"{filler}"}}]}}"#)
        }
    };
    Fixture {
        shape,
        target_bytes,
        text,
        node_count: 8,
        expected: None,
        max_depth: 4,
        max_sequence_items: 1,
        max_map_entries: 3,
        max_key_bytes: 7,
        max_string_bytes: target_bytes,
        max_number_bytes: 1,
        payload_bytes: target_bytes.saturating_mul(2),
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) payload: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Metadata {
    pub(crate) name: String,
    pub(crate) payload: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct WireEnvelope {
    pub(crate) version: u32,
    pub(crate) records: Vec<WireRecord>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct WireRecord {
    pub(crate) id: u64,
    pub(crate) payload: String,
}

pub(crate) fn fixture(shape: Shape, target_bytes: usize) -> Fixture {
    let filler = "x".repeat(target_bytes.saturating_sub(96).max(1));
    let text = match shape {
        Shape::Config => format!(r#"{{"name":"bench","enabled":true,"payload":"{filler}"}}"#),
        Shape::Metadata => format!(r#"{{"name":"bench","payload":"{filler}","region":"west"}}"#),
        Shape::WireEnvelope => {
            format!(r#"{{"version":1,"records":[{{"id":7,"payload":"{filler}"}}]}}"#)
        }
    };
    assert!(
        text.len() >= target_bytes.saturating_sub(128),
        "fixture is materially below requested size"
    );
    let node_count = match shape {
        Shape::Config | Shape::Metadata => 4,
        Shape::WireEnvelope => 6,
    };
    let filler_bytes = target_bytes.saturating_sub(96).max(1);
    let (max_depth, max_sequence_items, max_map_entries, max_key_bytes, max_number_bytes, payload_bytes) = match shape {
        Shape::Config => (2, 0, 3, 7, 0, filler_bytes + 23),
        Shape::Metadata => (2, 0, 3, 7, 0, filler_bytes + 26),
        Shape::WireEnvelope => (4, 1, 2, 7, 1, filler_bytes + 25),
    };
    let expected = match shape {
        Shape::Config => Expected::Config(serde_json::from_str(&text).expect("config fixture must parse")),
        Shape::Metadata => Expected::Metadata(serde_json::from_str(&text).expect("metadata fixture must parse")),
        Shape::WireEnvelope => Expected::WireEnvelope(serde_json::from_str(&text).expect("wire fixture must parse")),
    };
    let fixture = Fixture {
        shape,
        target_bytes,
        text,
        node_count,
        expected: Some(expected),
        max_depth,
        max_sequence_items,
        max_map_entries,
        max_key_bytes,
        max_string_bytes: filler_bytes.max(5),
        max_number_bytes,
        payload_bytes,
    };
    fixture.assert_meaning();
    fixture
}

impl Fixture {
    pub(crate) fn bytes(&self) -> usize {
        self.text.len()
    }

    fn assert_meaning(&self) {
        match self.shape {
            Shape::Config => {
                let Expected::Config(value) = self.expected.as_ref().expect("success fixture has expected value")
                else {
                    panic!("config expected value must match shape")
                };
                assert_eq!(value.name, "bench");
                assert!(value.enabled);
                assert_eq!(value.payload.len(), self.target_bytes.saturating_sub(96).max(1));
            }
            Shape::Metadata => {
                let Expected::Metadata(value) = self.expected.as_ref().expect("success fixture has expected value")
                else {
                    panic!("metadata expected value must match shape")
                };
                assert_eq!(value.name, "bench");
                assert_eq!(value.payload.len(), self.target_bytes.saturating_sub(96).max(1));
            }
            Shape::WireEnvelope => {
                let Expected::WireEnvelope(value) = self.expected.as_ref().expect("success fixture has expected value")
                else {
                    panic!("wire expected value must match shape")
                };
                assert_eq!(value.version, 1);
                assert_eq!(value.records.len(), 1);
                assert_eq!(value.records[0].id, 7);
                assert_eq!(
                    value.records[0].payload.len(),
                    self.target_bytes.saturating_sub(96).max(1)
                );
            }
        }
    }
}
