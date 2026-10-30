// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Transaction-aware Serde compound serializers.

use std::collections::BTreeSet;

use qubit_budget::json::JsonContainerKind;

use crate::internal::JsonPrivateProtocolState;
use crate::value::SafeJsonValue;

/// Tracks partial compound values, protocol payloads, and their nesting
/// context.
pub(in crate::value::json_value_encoder) enum CompoundState {
    /// Ordered sequence children remain protected until finalization succeeds.
    Sequence {
        /// Guarded children accumulated for this sequence.
        values: Vec<SafeJsonValue>,
        /// Depth of this sequence container; its children are encoded at `depth
        /// + 1`.
        depth: usize,
        /// Number of accepted children, checked before incrementing.
        count: usize,
        /// Sequence flavor used for budget admission.
        kind: JsonContainerKind,
    },
    /// Map entries and pending key state, retained transactionally.
    Map {
        /// Guarded values paired with their keys.
        values: Vec<(String, SafeJsonValue)>,
        /// Keys already accepted, used to reject duplicates.
        keys: BTreeSet<String>,
        /// Key awaiting exactly one value; cleared after successful insertion.
        next_key: Option<String>,
        /// Depth of this map container; its values are encoded at `depth + 1`.
        depth: usize,
        /// Number of accepted entries, checked before incrementing.
        count: usize,
    },
    /// Tuple enum variant with guarded positional children.
    TupleVariant {
        /// Variant name used by the encoded representation.
        variant: String,
        /// Guarded children retained until the variant succeeds.
        values: Vec<SafeJsonValue>,
        /// Depth of the outer enum-variant object. The inner container is at
        /// `depth + 1`, and its children are encoded at `depth + 2`.
        depth: usize,
        /// Accepted child count, checked against overflow and limits.
        count: usize,
    },
    /// Struct enum variant with guarded keyed children.
    StructVariant {
        /// Variant name used by the encoded representation.
        variant: String,
        /// Guarded values paired with their field names.
        values: Vec<(String, SafeJsonValue)>,
        /// Field names already accepted, used to reject duplicates.
        keys: BTreeSet<String>,
        /// Depth of the outer enum-variant object. The inner container is at
        /// `depth + 1`, and its children are encoded at `depth + 2`.
        depth: usize,
        /// Accepted field count, checked before incrementing.
        count: usize,
    },
    /// Collected payload and exact private protocol state for RawValue.
    RawValue {
        /// Complete payload text, validated only after collection.
        text: Option<String>,
        /// Sticky protocol state; malformed lifecycle errors are preserved.
        state: JsonPrivateProtocolState,
        /// Depth at which the raw value is materialized.
        depth: usize,
    },
    /// Collected payload and exact private protocol state for Number.
    Number {
        /// Number payload text, validated after protocol completion.
        text: Option<String>,
        /// Sticky protocol state; malformed lifecycle errors are preserved.
        state: JsonPrivateProtocolState,
        /// Depth at which the number is materialized.
        depth: usize,
    },
}
