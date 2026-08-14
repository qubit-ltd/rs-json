// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for budget-aware JSON encoding.

use std::cell::Cell;
use std::io;
use std::io::Write;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::text::JsonEncodeError;
use qubit_json::text::encode_to_vec;
use qubit_json::text::encode_to_writer;
use qubit_json::text::encode_to_writer_incremental;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as _;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;
use serde_json::Number;
use serde_json::json;
use serde_json::value::RawValue;

use super::json_test_limits_tests::JsonTestLimits;

/// Arbitrary-precision number text used by online accounting tests.
const LARGE_NUMBER_TEXT: &str = "123456789012345678901234567890";

/// Private serde_json protocol token used by arbitrary-precision numbers.
const JSON_NUMBER_TOKEN: &str =
    concat!("$", "serde_json", ":", ":private::Number");

/// Private serde_json protocol token used by raw JSON fragments.
const JSON_RAW_VALUE_TOKEN: &str =
    concat!("$", "serde_json", ":", ":private::RawValue");

/// Value that emits a prefix before returning a custom Serde error.
struct FailsAfterPrefix;

impl Serialize for FailsAfterPrefix {
    /// Emits one sequence item, then fails before completing the sequence.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        sequence.serialize_element(&1_u8)?;
        Err(S::Error::custom("deliberate serialization failure"))
    }
}

/// Unknown-length sequence with observable element traversal.
struct CountedSequence<'a> {
    /// Number of elements whose serialization was entered.
    serialized: &'a Cell<usize>,

    /// Number of elements offered by the source.
    len: usize,
}

impl Serialize for CountedSequence<'_> {
    /// Emits an unknown-length sequence while recording each entered element.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for value in 0..self.len {
            self.serialized.set(self.serialized.get() + 1);
            sequence.serialize_element(&value)?;
        }
        sequence.end()
    }
}

/// Recursively emitted sequence with observable traversal depth.
struct CountedDepth<'a> {
    /// Number of values whose serialization was entered.
    serialized: &'a Cell<usize>,

    /// Nested sequence levels remaining below this value.
    remaining: usize,
}

impl Serialize for CountedDepth<'_> {
    /// Emits one nested child until the requested source depth is reached.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.serialized.set(self.serialized.get() + 1);
        if self.remaining == 0 {
            serializer.serialize_unit()
        } else {
            let mut sequence = serializer.serialize_seq(Some(1))?;
            sequence.serialize_element(&CountedDepth {
                serialized: self.serialized,
                remaining: self.remaining - 1,
            })?;
            sequence.end()
        }
    }
}

/// Sequence containing one checked value before an observable tail.
struct SequenceThenTail<'a, T: ?Sized> {
    /// Value expected to fail a budget check.
    first: &'a T,

    /// Number of times serialization reached the tail.
    serialized_tail: &'a Cell<usize>,
}

impl<T> Serialize for SequenceThenTail<'_, T>
where
    T: Serialize + ?Sized,
{
    /// Emits the checked value before recording entry into the tail.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element(self.first)?;
        self.serialized_tail.set(self.serialized_tail.get() + 1);
        sequence.serialize_element(&())?;
        sequence.end()
    }
}

/// Map containing one checked entry before an observable tail.
struct MapThenTail<'a, K: ?Sized, V: ?Sized> {
    /// Key expected to pass or fail its budget check.
    key: &'a K,

    /// Value expected to pass or fail its budget check.
    value: &'a V,

    /// Number of times serialization reached the tail entry.
    serialized_tail: &'a Cell<usize>,
}

impl<K, V> Serialize for MapThenTail<'_, K, V>
where
    K: Serialize + ?Sized,
    V: Serialize + ?Sized,
{
    /// Emits the checked entry before recording entry into the tail.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry(self.key, self.value)?;
        self.serialized_tail.set(self.serialized_tail.get() + 1);
        map.serialize_entry("tail", &())?;
        map.end()
    }
}

/// Map that intentionally omits its entry-count hint.
struct UnknownMap(usize);

impl Serialize for UnknownMap {
    /// Emits integer key/value pairs through an unknown-length map.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        for value in 0..self.0 {
            map.serialize_entry(&value, &value)?;
        }
        map.end()
    }
}

/// Writer that accepts a bounded prefix, then fails every later write.
struct PrefixThenFailWriter {
    /// Bytes accepted before the configured failure boundary.
    bytes: Vec<u8>,

    /// Maximum number of bytes accepted across all writes.
    accepted: usize,
}

impl Write for PrefixThenFailWriter {
    /// Accepts at most the remaining prefix capacity, then reports an error.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let remaining = self.accepted.saturating_sub(self.bytes.len());
        if remaining == 0 {
            return Err(io::Error::other("deliberate writer failure"));
        }
        let accepted = remaining.min(input.len());
        self.bytes.extend_from_slice(&input[..accepted]);
        Ok(accepted)
    }

    /// Flushes without additional side effects.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Asserts a budget error occurs before traversal reaches a source tail.
fn assert_online_rejection<T>(
    value: &T,
    limits: JsonTestLimits,
    expected: JsonResource,
    serialized_tail: &Cell<usize>,
) where
    T: Serialize + ?Sized,
{
    let mut session = limits.encode_session();
    let error = encode_to_vec(value, &mut session)
        .expect_err("the first value must be rejected online");
    let JsonEncodeError::Budget(error) = error else {
        panic!("expected a budget error, got {error:?}");
    };
    assert_eq!(
        error
            .budget_error()
            .expect("the error must contain a budget failure")
            .resource(),
        &expected,
    );
    assert_eq!(serialized_tail.get(), 0);
}

struct ScalarSerializerSurface;

macro_rules! define_scalar_serializer_surface {
    ($name:ident, $method:ident, $($value:expr),+ $(,)?) => {
        struct $name;

        impl Serialize for $name {
            /// Delegates the fixture to one less-common Serde scalar method.
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.$method($($value),+)
            }
        }
    };
}

define_scalar_serializer_surface!(F32Surface, serialize_f32, 1.5_f32);
define_scalar_serializer_surface!(F64Surface, serialize_f64, 1.5_f64);
define_scalar_serializer_surface!(BytesSurface, serialize_bytes, &[1_u8, 2]);
define_scalar_serializer_surface!(
    UnitStructSurface,
    serialize_unit_struct,
    "Unit"
);
define_scalar_serializer_surface!(
    UnitVariantSurface,
    serialize_unit_variant,
    "Kind",
    0,
    "Unit"
);
define_scalar_serializer_surface!(
    NewtypeStructSurface,
    serialize_newtype_struct,
    "Value",
    &1_u8
);
define_scalar_serializer_surface!(
    NewtypeVariantSurface,
    serialize_newtype_variant,
    "Kind",
    0,
    "Value",
    &1_u8
);

impl Serialize for ScalarSerializerSurface {
    /// Formats a display-only value through Serde's collection hook.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&1_234)
    }
}

struct TupleStructSurface;

impl Serialize for TupleStructSurface {
    /// Emits one tuple-struct field through the budgeted compound adapter.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serializer.serialize_tuple_struct("Tuple", 1)?;
        value.serialize_field(&1_u8)?;
        value.end()
    }
}

struct TupleVariantSurface;

impl Serialize for TupleVariantSurface {
    /// Emits one tuple-variant field through the budgeted compound adapter.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value =
            serializer.serialize_tuple_variant("Kind", 0, "Tuple", 1)?;
        value.serialize_field(&1_u8)?;
        value.end()
    }
}

struct StructSurface;

impl Serialize for StructSurface {
    /// Exercises regular and skipped struct fields.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serializer.serialize_struct("Struct", 1)?;
        value.skip_field("ignored")?;
        value.serialize_field("value", &1_u8)?;
        value.end()
    }
}

struct StructVariantSurface;

impl Serialize for StructVariantSurface {
    /// Exercises regular and skipped struct-variant fields.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value =
            serializer.serialize_struct_variant("Kind", 0, "Struct", 1)?;
        value.skip_field("ignored")?;
        value.serialize_field("value", &1_u8)?;
        value.end()
    }
}

/// Selects one Serde scalar or compound operation for a map key.
#[derive(Clone, Copy)]
enum KeySurfaceKind {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    Char,
    Str,
    Bytes,
    None,
    Some,
    Unit,
    UnitStruct,
    UnitVariant,
    NewtypeStruct,
    NewtypeVariant,
    Seq,
    Tuple,
    TupleStruct,
    TupleVariant,
    Map,
    Struct,
    StructVariant,
    CollectStr,
}

/// Exercises the key serializer's complete Serde forwarding surface.
struct KeySurface(KeySurfaceKind);

impl Serialize for KeySurface {
    /// Calls one selected serializer operation, including unsupported JSON-key
    /// shapes whose forwarding code still needs to remain covered.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            KeySurfaceKind::Bool => serializer.serialize_bool(true),
            KeySurfaceKind::I8 => serializer.serialize_i8(-1),
            KeySurfaceKind::I16 => serializer.serialize_i16(-1),
            KeySurfaceKind::I32 => serializer.serialize_i32(-1),
            KeySurfaceKind::I64 => serializer.serialize_i64(-1),
            KeySurfaceKind::I128 => serializer.serialize_i128(-1),
            KeySurfaceKind::U8 => serializer.serialize_u8(1),
            KeySurfaceKind::U16 => serializer.serialize_u16(1),
            KeySurfaceKind::U32 => serializer.serialize_u32(1),
            KeySurfaceKind::U64 => serializer.serialize_u64(1),
            KeySurfaceKind::U128 => serializer.serialize_u128(1),
            KeySurfaceKind::F32 => serializer.serialize_f32(1.5),
            KeySurfaceKind::F64 => serializer.serialize_f64(1.5),
            KeySurfaceKind::Char => serializer.serialize_char('x'),
            KeySurfaceKind::Str => serializer.serialize_str("key"),
            KeySurfaceKind::Bytes => serializer.serialize_bytes(&[1, 2]),
            KeySurfaceKind::None => serializer.serialize_none(),
            KeySurfaceKind::Some => serializer.serialize_some(&1_u8),
            KeySurfaceKind::Unit => serializer.serialize_unit(),
            KeySurfaceKind::UnitStruct => {
                serializer.serialize_unit_struct("KeyUnit")
            }
            KeySurfaceKind::UnitVariant => {
                serializer.serialize_unit_variant("Key", 0, "Unit")
            }
            KeySurfaceKind::NewtypeStruct => {
                serializer.serialize_newtype_struct("KeyValue", &1_u8)
            }
            KeySurfaceKind::NewtypeVariant => {
                serializer.serialize_newtype_variant("Key", 0, "Value", &1_u8)
            }
            KeySurfaceKind::Seq => {
                let mut value = serializer.serialize_seq(Some(1))?;
                value.serialize_element(&1_u8)?;
                value.end()
            }
            KeySurfaceKind::Tuple => {
                let mut value = serializer.serialize_tuple(1)?;
                value.serialize_element(&1_u8)?;
                value.end()
            }
            KeySurfaceKind::TupleStruct => {
                let mut value = serializer.serialize_tuple_struct("Key", 1)?;
                value.serialize_field(&1_u8)?;
                value.end()
            }
            KeySurfaceKind::TupleVariant => {
                let mut value =
                    serializer.serialize_tuple_variant("Key", 0, "Value", 1)?;
                value.serialize_field(&1_u8)?;
                value.end()
            }
            KeySurfaceKind::Map => {
                let mut value = serializer.serialize_map(Some(1))?;
                value.serialize_entry("nested", &1_u8)?;
                value.end()
            }
            KeySurfaceKind::Struct => {
                let mut value = serializer.serialize_struct("Key", 1)?;
                value.serialize_field("value", &1_u8)?;
                value.end()
            }
            KeySurfaceKind::StructVariant => {
                let mut value = serializer
                    .serialize_struct_variant("Key", 0, "Value", 1)?;
                value.serialize_field("value", &1_u8)?;
                value.end()
            }
            KeySurfaceKind::CollectStr => serializer.collect_str(&1_234),
        }
    }
}

/// Emits one custom key so a selected key serializer operation is entered.
struct OneKey<'a> {
    /// Key fixture selected for this test case.
    key: &'a KeySurface,
}

impl Serialize for OneKey<'_> {
    /// Emits one map entry with the selected custom key.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(self.key, &1_u8)?;
        map.end()
    }
}

/// Selects one delegated operation of the private serde_json text serializer.
#[derive(Clone, Copy)]
enum PrivateTextSurfaceKind {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    Char,
    Bytes,
    None,
    Some,
    Unit,
    UnitStruct,
    UnitVariant,
    NewtypeStruct,
    NewtypeVariant,
    Seq,
    Tuple,
    TupleStruct,
    TupleVariant,
    Map,
    Struct,
    StructVariant,
    CollectStr,
}

/// Emits a serde_json private number shape around a selected field value.
struct PrivateTextSurface {
    /// Selects the private serde_json protocol to exercise.
    token: &'static str,

    /// Selects the delegated field operation.
    kind: PrivateTextSurfaceKind,
}

impl Serialize for PrivateTextSurface {
    /// Enters the private-number protocol used by arbitrary-precision values.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serializer.serialize_struct(self.token, 1)?;
        value.serialize_field(self.token, &PrivateTextValue(self.kind))?;
        value.end()
    }
}

/// Scalar emitted below a nested private payload boundary.
struct NestedPrivateScalar(bool);

impl Serialize for NestedPrivateScalar {
    /// Enters the private serializer through a budgeted child wrapper.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.0 {
            serializer.serialize_f32(1.5)
        } else {
            serializer.serialize_f64(1.5)
        }
    }
}

/// Emits one selected operation into the private text serializer.
struct PrivateTextValue(PrivateTextSurfaceKind);

impl Serialize for PrivateTextValue {
    /// Calls one operation forwarded by the private text serializer.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            PrivateTextSurfaceKind::Bool => serializer.serialize_bool(true),
            PrivateTextSurfaceKind::I8 => serializer.serialize_i8(-1),
            PrivateTextSurfaceKind::I16 => serializer.serialize_i16(-1),
            PrivateTextSurfaceKind::I32 => serializer.serialize_i32(-1),
            PrivateTextSurfaceKind::I64 => serializer.serialize_i64(-1),
            PrivateTextSurfaceKind::I128 => serializer.serialize_i128(-1),
            PrivateTextSurfaceKind::U8 => serializer.serialize_u8(1),
            PrivateTextSurfaceKind::U16 => serializer.serialize_u16(1),
            PrivateTextSurfaceKind::U32 => serializer.serialize_u32(1),
            PrivateTextSurfaceKind::U64 => serializer.serialize_u64(1),
            PrivateTextSurfaceKind::U128 => serializer.serialize_u128(1),
            PrivateTextSurfaceKind::F32 => serializer.serialize_f32(1.5),
            PrivateTextSurfaceKind::F64 => serializer.serialize_f64(1.5),
            PrivateTextSurfaceKind::Char => serializer.serialize_char('x'),
            PrivateTextSurfaceKind::Bytes => {
                serializer.serialize_bytes(&[1, 2])
            }
            PrivateTextSurfaceKind::None => serializer.serialize_none(),
            PrivateTextSurfaceKind::Some => serializer.serialize_some(&1_u8),
            PrivateTextSurfaceKind::Unit => serializer.serialize_unit(),
            PrivateTextSurfaceKind::UnitStruct => {
                serializer.serialize_unit_struct("Value")
            }
            PrivateTextSurfaceKind::UnitVariant => {
                serializer.serialize_unit_variant("Value", 0, "Unit")
            }
            PrivateTextSurfaceKind::NewtypeStruct => {
                serializer.serialize_newtype_struct("Value", &1_u8)
            }
            PrivateTextSurfaceKind::NewtypeVariant => serializer
                .serialize_newtype_variant("Value", 0, "Nested", &1_u8),
            PrivateTextSurfaceKind::Seq => {
                let mut value = serializer.serialize_seq(Some(1))?;
                value.serialize_element(&1_u8)?;
                value.end()
            }
            PrivateTextSurfaceKind::Tuple => {
                let mut value = serializer.serialize_tuple(1)?;
                value.serialize_element(&1_u8)?;
                value.end()
            }
            PrivateTextSurfaceKind::TupleStruct => {
                let mut value =
                    serializer.serialize_tuple_struct("Value", 1)?;
                value.serialize_field(&1_u8)?;
                value.end()
            }
            PrivateTextSurfaceKind::TupleVariant => {
                let mut value = serializer
                    .serialize_tuple_variant("Value", 0, "Nested", 1)?;
                value.serialize_field(&1_u8)?;
                value.end()
            }
            PrivateTextSurfaceKind::Map => {
                let mut value = serializer.serialize_map(Some(1))?;
                value.serialize_entry("nested", &1_u8)?;
                value.end()
            }
            PrivateTextSurfaceKind::Struct => {
                let mut value = serializer.serialize_struct("Value", 1)?;
                value.serialize_field("value", &1_u8)?;
                value.end()
            }
            PrivateTextSurfaceKind::StructVariant => {
                let mut value = serializer
                    .serialize_struct_variant("Value", 0, "Nested", 1)?;
                value.serialize_field("value", &1_u8)?;
                value.end()
            }
            PrivateTextSurfaceKind::CollectStr => {
                serializer.collect_str(&1_234)
            }
        }
    }
}

/// Verifies less-common Serde serializer entry points remain budget-aware.
#[test]
fn test_encode_exercises_all_serializer_entry_points() {
    fn assert_encodes<T: Serialize>(value: &T) {
        let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
        encode_to_vec(value, &mut session)
            .expect("every supported Serde entry point must encode");
    }

    assert_encodes(&F32Surface);
    assert_encodes(&F64Surface);
    assert_encodes(&BytesSurface);
    assert_encodes(&UnitStructSurface);
    assert_encodes(&UnitVariantSurface);
    assert_encodes(&NewtypeStructSurface);
    assert_encodes(&NewtypeVariantSurface);
    assert_encodes(&ScalarSerializerSurface);
    assert_encodes(&TupleStructSurface);
    assert_encodes(&TupleVariantSurface);
    assert_encodes(&StructSurface);
    assert_encodes(&StructVariantSurface);
    assert_encodes(&1_u8);
    assert_encodes(&vec![1_u8]);

    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    encode_to_vec(&f32::NAN, &mut session)
        .expect("non-finite f32 should use the JSON null path");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    encode_to_vec(&f64::NAN, &mut session)
        .expect("non-finite f64 should use the JSON null path");
}

/// Verifies every map-key forwarding method performs its budget check before
/// delegating to serde_json.
#[test]
fn test_encode_exercises_all_key_serializer_entry_points() {
    let kinds = [
        KeySurfaceKind::Bool,
        KeySurfaceKind::I8,
        KeySurfaceKind::I16,
        KeySurfaceKind::I32,
        KeySurfaceKind::I64,
        KeySurfaceKind::I128,
        KeySurfaceKind::U8,
        KeySurfaceKind::U16,
        KeySurfaceKind::U32,
        KeySurfaceKind::U64,
        KeySurfaceKind::U128,
        KeySurfaceKind::F32,
        KeySurfaceKind::F64,
        KeySurfaceKind::Char,
        KeySurfaceKind::Str,
        KeySurfaceKind::Bytes,
        KeySurfaceKind::None,
        KeySurfaceKind::Some,
        KeySurfaceKind::Unit,
        KeySurfaceKind::UnitStruct,
        KeySurfaceKind::UnitVariant,
        KeySurfaceKind::NewtypeStruct,
        KeySurfaceKind::NewtypeVariant,
        KeySurfaceKind::Seq,
        KeySurfaceKind::Tuple,
        KeySurfaceKind::TupleStruct,
        KeySurfaceKind::TupleVariant,
        KeySurfaceKind::Map,
        KeySurfaceKind::Struct,
        KeySurfaceKind::StructVariant,
        KeySurfaceKind::CollectStr,
    ];

    for kind in kinds {
        let key = KeySurface(kind);
        let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
        let _ = encode_to_vec(&OneKey { key: &key }, &mut session);
    }

    for kind in kinds {
        let key = KeySurface(kind);
        let mut output = Vec::new();
        let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
        let _ = encode_to_writer_incremental(
            &mut output,
            &OneKey { key: &key },
            &mut session,
        );
    }
}

/// Verifies private serde_json payload forwarding remains total for all Serde
/// operations, including shapes rejected by the underlying JSON serializer.
#[test]
fn test_encode_exercises_private_text_serializer_entry_points() {
    let kinds = [
        PrivateTextSurfaceKind::Bool,
        PrivateTextSurfaceKind::I8,
        PrivateTextSurfaceKind::I16,
        PrivateTextSurfaceKind::I32,
        PrivateTextSurfaceKind::I64,
        PrivateTextSurfaceKind::I128,
        PrivateTextSurfaceKind::U8,
        PrivateTextSurfaceKind::U16,
        PrivateTextSurfaceKind::U32,
        PrivateTextSurfaceKind::U64,
        PrivateTextSurfaceKind::U128,
        PrivateTextSurfaceKind::F32,
        PrivateTextSurfaceKind::F64,
        PrivateTextSurfaceKind::Char,
        PrivateTextSurfaceKind::Bytes,
        PrivateTextSurfaceKind::None,
        PrivateTextSurfaceKind::Some,
        PrivateTextSurfaceKind::Unit,
        PrivateTextSurfaceKind::UnitStruct,
        PrivateTextSurfaceKind::UnitVariant,
        PrivateTextSurfaceKind::NewtypeStruct,
        PrivateTextSurfaceKind::NewtypeVariant,
        PrivateTextSurfaceKind::Seq,
        PrivateTextSurfaceKind::Tuple,
        PrivateTextSurfaceKind::TupleStruct,
        PrivateTextSurfaceKind::TupleVariant,
        PrivateTextSurfaceKind::Map,
        PrivateTextSurfaceKind::Struct,
        PrivateTextSurfaceKind::StructVariant,
        PrivateTextSurfaceKind::CollectStr,
    ];

    for kind in kinds {
        let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
        let _ = encode_to_vec(
            &PrivateTextSurface {
                token: JSON_NUMBER_TOKEN,
                kind,
            },
            &mut session,
        );
        let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
        let _ = encode_to_vec(
            &PrivateTextSurface {
                token: JSON_RAW_VALUE_TOKEN,
                kind,
            },
            &mut session,
        );
    }

    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let _ = encode_to_vec(&vec![NestedPrivateScalar(true)], &mut session);
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let _ = encode_to_vec(&vec![NestedPrivateScalar(false)], &mut session);
}

/// Verifies a budget failure leaves the destination writer unchanged.
#[test]
fn test_encode_to_writer_failure_does_not_touch_external_writer() {
    let limits = JsonEncodeLimits::empty().with_output_bytes_limit(
        ResourceLimit::new(JsonResource::OutputBytes, 3),
    );
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = encode_to_writer(&mut output, &"long", &mut session)
        .expect_err("the encoded string must exceed the output budget");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(output.is_empty());
}

/// Verifies a RawValue is traversed once and emitted without metadata charges.
#[test]
fn test_encode_to_vec_counts_raw_value_once() {
    let raw = RawValue::from_string(String::from(r#"{"k":"v"}"#))
        .expect("the fixture must be valid raw JSON");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());

    let output = encode_to_vec(raw.as_ref(), &mut session)
        .expect("the raw JSON value must encode");

    assert_eq!(output, br#"{"k":"v"}"#);
}

/// Verifies a custom Serde failure does not commit its buffered prefix.
#[test]
fn test_encode_to_writer_serde_failure_does_not_touch_external_writer() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut output = Vec::new();

    let error = encode_to_writer(&mut output, &FailsAfterPrefix, &mut session)
        .expect_err("the custom serializer must fail");

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    assert!(output.is_empty());
}

/// Verifies a failed encode does not consume borrowed output budget capacity.
#[test]
fn test_encode_to_vec_serde_failure_does_not_consume_output_budget() {
    let value_limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty()
            .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
    );
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(
            JsonResource::OutputBytes,
            16,
        ))
        .with_value_limits(value_limits);
    let mut session = JsonEncodeSession::owned(limits);
    encode_to_vec(&true, &mut session).expect("the initial value should fit");
    let used_before = session
        .output_budget()
        .expect("output accounting should be configured")
        .used();
    let nodes_before = session.value_budget().structure_budget().used_nodes();

    let error = encode_to_vec(&FailsAfterPrefix, &mut session)
        .expect_err("the custom serializer must fail");

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    let output = session
        .output_budget()
        .expect("output accounting should remain configured");
    assert_eq!(output.used(), used_before);
    assert_eq!(output.remaining(), 16 - used_before);
    assert!(
        session.value_budget().structure_budget().used_nodes() > nodes_before,
        "accepted value work remains charged after serialization failure",
    );
}

/// Verifies a known map length is checked before its entries are traversed.
#[test]
fn test_encode_to_vec_known_map_limit_stops_before_source_tail() {
    let serialized_tail = Cell::new(0);
    let value = MapThenTail {
        key: &"first",
        value: &1_u8,
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_map_entries(1),
        JsonResource::MapEntries,
        &serialized_tail,
    );
}

/// Verifies output rejection stops traversal before a long source tail.
#[test]
fn test_encode_to_vec_output_limit_stops_before_source_tail() {
    let serialized = Cell::new(0);
    let value = CountedSequence {
        serialized: &serialized,
        len: 1_000,
    };
    let limits = JsonEncodeLimits::empty().with_output_bytes_limit(
        ResourceLimit::new(JsonResource::OutputBytes, 8),
    );
    let mut session = JsonEncodeSession::owned(limits);

    let error = encode_to_vec(&value, &mut session)
        .expect_err("the output budget must reject the long sequence");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < value.len);
}

/// Verifies final writer I/O can leave an accepted prefix in the destination.
#[test]
fn test_encode_to_writer_io_failure_can_leave_partial_output() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut writer = PrefixThenFailWriter {
        bytes: Vec::new(),
        accepted: 2,
    };

    let error = encode_to_writer(&mut writer, &[1_u8, 2_u8], &mut session)
        .expect_err("the destination writer must fail during final commit");

    assert!(matches!(error, JsonEncodeError::Write(_)));
    assert_eq!(writer.bytes, b"[1");
}

/// Verifies incremental encoding matches transactional encoding on success.
#[test]
fn test_encode_to_writer_incremental_matches_encode_to_vec() {
    let value = json!({"items": [1, true, "text"]});
    let mut expected_session =
        JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let expected = encode_to_vec(&value, &mut expected_session)
        .expect("transactional encoding should succeed");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut output = Vec::new();

    encode_to_writer_incremental(&mut output, &value, &mut session)
        .expect("incremental encoding should succeed");

    assert_eq!(output, expected);
}

/// Verifies incremental encoding may leave accepted output on a budget error.
#[test]
fn test_encode_to_writer_incremental_preserves_partial_output_on_budget_error()
{
    let limits = JsonEncodeLimits::empty().with_output_bytes_limit(
        ResourceLimit::new(JsonResource::OutputBytes, 4),
    );
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = encode_to_writer_incremental(
        &mut output,
        &json!([1, 2, 3]),
        &mut session,
    )
    .expect_err("the output limit must reject the document");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert_eq!(output, b"[1,2");
    assert_eq!(
        session
            .output_budget()
            .expect("output budget should remain configured")
            .used(),
        4,
    );
}

/// Verifies incremental encoding maps destination failures to `Write`.
#[test]
fn test_encode_to_writer_incremental_preserves_partial_output_on_io_error() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut writer = PrefixThenFailWriter {
        bytes: Vec::new(),
        accepted: 2,
    };

    let error =
        encode_to_writer_incremental(&mut writer, &[1_u8, 2_u8], &mut session)
            .expect_err("the destination writer must fail incrementally");

    assert!(matches!(error, JsonEncodeError::Write(_)));
    assert_eq!(writer.bytes, b"[1");
}

/// Verifies incremental Serde failures retain their accepted prefix and usage.
#[test]
fn test_encode_to_writer_incremental_preserves_partial_output_on_serde_error() {
    let limits = JsonEncodeLimits::empty().with_output_bytes_limit(
        ResourceLimit::new(JsonResource::OutputBytes, 16),
    );
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = encode_to_writer_incremental(
        &mut output,
        &FailsAfterPrefix,
        &mut session,
    )
    .expect_err("the custom serializer must fail after its prefix");

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    assert_eq!(output, b"[1");
    assert_eq!(
        session
            .output_budget()
            .expect("output budget should remain configured")
            .used(),
        2,
    );
}

/// Verifies node accounting stops a long source before it is exhausted.
#[test]
fn test_encode_to_vec_node_limit_stops_before_source_tail() {
    let serialized = Cell::new(0);
    let value = CountedSequence {
        serialized: &serialized,
        len: 1_000,
    };
    let mut session = JsonTestLimits::new().with_max_nodes(3).encode_session();

    let error = encode_to_vec(&value, &mut session)
        .expect_err("the node budget must reject the long sequence");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < value.len);
}

/// Verifies depth accounting stops recursive serialization online.
#[test]
fn test_encode_to_vec_depth_limit_stops_before_source_tail() {
    const SOURCE_DEPTH: usize = 128;

    let serialized = Cell::new(0);
    let value = CountedDepth {
        serialized: &serialized,
        remaining: SOURCE_DEPTH - 1,
    };
    let mut session = JsonTestLimits::new().with_max_depth(4).encode_session();

    let error = encode_to_vec(&value, &mut session)
        .expect_err("the depth budget must reject recursive serialization");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < SOURCE_DEPTH);
}

/// Verifies arbitrary-precision number rejection occurs before a later value.
#[test]
fn test_encode_to_vec_number_limit_stops_before_source_tail() {
    let number = LARGE_NUMBER_TEXT
        .parse::<Number>()
        .expect("the number fixture must parse");
    let serialized_tail = Cell::new(0);
    let value = SequenceThenTail {
        first: &number,
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new()
            .with_max_number_bytes(LARGE_NUMBER_TEXT.len() - 1),
        JsonResource::NumberBytes,
        &serialized_tail,
    );
}

/// Verifies UTF-8 key rejection occurs before a later map entry.
#[test]
fn test_encode_to_vec_key_limit_stops_before_source_tail() {
    let key = "a\n\"你";
    let serialized_tail = Cell::new(0);
    let value = MapThenTail {
        key: &key,
        value: &(),
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_key_bytes(key.len() - 1),
        JsonResource::KeyBytes,
        &serialized_tail,
    );
}

/// Verifies UTF-8 string rejection occurs before a later sequence value.
#[test]
fn test_encode_to_vec_string_limit_stops_before_source_tail() {
    let text = "x\t好";
    let serialized_tail = Cell::new(0);
    let value = SequenceThenTail {
        first: &text,
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_string_bytes(text.len() - 1),
        JsonResource::StringBytes,
        &serialized_tail,
    );
}

/// Verifies an unknown map rejects at its actual online boundary.
#[test]
fn test_encode_to_vec_unknown_map_limit_stops_before_source_tail() {
    let map = UnknownMap(2);
    let serialized_tail = Cell::new(0);
    let value = SequenceThenTail {
        first: &map,
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_map_entries(1),
        JsonResource::MapEntries,
        &serialized_tail,
    );
}
