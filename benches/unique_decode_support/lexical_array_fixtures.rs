// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic lexical array inputs shared by focused allocation and latency
//! benches.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArrayShape {
    Empty,
    Scalar,
    Records,
    Chain,
}

impl ArrayShape {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty_array",
            Self::Scalar => "wide_scalar_array",
            Self::Records => "record_array",
            Self::Chain => "single_child_chain",
        }
    }
}

pub(crate) const SIZES: [usize; 4] = [1, 32, 512, 4096];
pub(crate) const RECORD_SIZES: [usize; 3] = [32, 512, 4096];
pub(crate) const CHAIN_DEPTHS: [usize; 4] = [8, 64, 256, 4096];

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Record {
    pub(crate) id: u64,
    pub(crate) label: String,
}

#[derive(Debug)]
pub(crate) struct ArrayFixture {
    pub(crate) shape: ArrayShape,
    pub(crate) size: usize,
    pub(crate) text: String,
    pub(crate) nodes: usize,
    pub(crate) depth: usize,
    pub(crate) sequence_items: usize,
}

impl ArrayFixture {
    pub(crate) fn empty() -> Self {
        Self {
            shape: ArrayShape::Empty,
            size: 0,
            text: "[]".into(),
            nodes: 1,
            depth: 1,
            sequence_items: 0,
        }
    }

    pub(crate) fn scalars(size: usize) -> Self {
        assert!(SIZES.contains(&size));
        let values = (0..size).map(|n| n.to_string()).collect::<Vec<_>>().join(",");
        Self {
            shape: ArrayShape::Scalar,
            size,
            text: format!("[{values}]"),
            nodes: size + 1,
            depth: 2,
            sequence_items: size,
        }
    }

    pub(crate) fn records(size: usize) -> Self {
        assert!(RECORD_SIZES.contains(&size));
        let values = (0..size)
            .map(|n| format!(r#"{{"id":{n},"label":"v{n:06}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        Self {
            shape: ArrayShape::Records,
            size,
            text: format!("[{values}]"),
            nodes: size * 3 + 1,
            depth: 3,
            sequence_items: size,
        }
    }

    pub(crate) fn chain(depth: usize) -> Self {
        assert!(CHAIN_DEPTHS.contains(&depth));
        let mut source = String::with_capacity(depth * 2 + 1);
        source.extend(std::iter::repeat_n('[', depth));
        source.push('0');
        source.extend(std::iter::repeat_n(']', depth));
        Self {
            shape: ArrayShape::Chain,
            size: depth,
            text: source,
            nodes: depth + 1,
            depth: depth + 1,
            sequence_items: 1,
        }
    }
}
