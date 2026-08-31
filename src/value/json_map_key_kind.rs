// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines unsupported Serde shapes used as JSON object keys.

/// Serde value shape that cannot be represented as a JSON object key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonMapKeyKind {
    /// A byte sequence.
    Bytes,
    /// An absent optional value.
    None,
    /// A unit value.
    Unit,
    /// A unit struct.
    UnitStruct,
    /// A newtype variant.
    NewtypeVariant,
    /// A sequence.
    Sequence,
    /// A tuple.
    Tuple,
    /// A tuple struct.
    TupleStruct,
    /// A tuple variant.
    TupleVariant,
    /// A map.
    Map,
    /// A struct.
    Struct,
    /// A struct variant.
    StructVariant,
}
