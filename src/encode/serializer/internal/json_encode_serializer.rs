// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde serializer decorator with online JSON budget checks.

use std::cell::RefCell;
use std::fmt::Display;

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error;

use super::BudgetedValue;
use super::DisplayBudgetKind;
use super::JsonEncodeCompound;
use super::JsonEncodeContext;
use crate::encode::JsonIntegerSignedness;
use crate::encode::JsonSerializationErrorKind;
use crate::internal::JsonLexemeLength;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;
use crate::internal::SERDE_JSON_RAW_VALUE_TOKEN;

/// Decorates one Serde serializer with eager JSON budget checks.
///
/// # Type Parameters
///
/// * `'transaction` - Lifetime of the shared value transaction.
/// * `'budget` - Lifetime of the budget borrowed by that transaction.
/// * `'context` - Lifetime of the shared traversal context borrow.
/// * `S` - Underlying serializer receiving the delegated JSON events.
/// * `R` - Resource identifier used by budget errors.
/// * `Q` - Resource quantity type used by budget measurements.
/// * `VALUE_LIMITS` - Whether decoded-value limits are enforced.
pub(in crate::encode) struct JsonEncodeSerializer<
    'transaction,
    'budget,
    'context,
    S,
    R,
    Q,
    const VALUE_LIMITS: bool,
> where
    Q: ResourceQuantity,
{
    /// Underlying serializer that emits JSON events.
    inner: S,

    /// Shared mutable state for this traversal.
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Root-inclusive depth assigned to the current value.
    depth: usize,
}

impl<'transaction, 'budget, 'context, S, R, Q, const VALUE_LIMITS: bool>
    JsonEncodeSerializer<'transaction, 'budget, 'context, S, R, Q, VALUE_LIMITS>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a root decorator at depth one.
    ///
    /// # Parameters
    ///
    /// * `inner` - Underlying serializer that emits the JSON document.
    /// * `context` - Shared state for value and output accounting.
    ///
    /// # Returns
    ///
    /// A decorator positioned at the root JSON value.
    #[must_use]
    #[inline]
    pub(in crate::encode) fn new(
        inner: S,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
    ) -> Self {
        Self {
            inner,
            context,
            depth: 1,
        }
    }

    /// Creates a nested decorator sharing an existing traversal context.
    ///
    /// # Parameters
    ///
    /// * `inner` - Underlying serializer that emits the JSON document.
    /// * `context` - Shared state for value and output accounting.
    /// * `depth` - Root-inclusive depth assigned to the nested value.
    ///
    /// # Returns
    ///
    /// A decorator sharing the supplied traversal context at `depth`.
    #[must_use]
    #[inline]
    pub(super) const fn with_context(
        inner: S,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        depth: usize,
    ) -> Self {
        Self {
            inner,
            context,
            depth,
        }
    }

    /// Stages one complete JSON measurement when value limits are enabled.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serializer error type used to report rejected measurements.
    ///
    /// # Parameters
    ///
    /// * `measurement` - JSON node or payload measurement to admit.
    ///
    /// # Returns
    ///
    /// Unit after the measurement is admitted or value limits are disabled.
    ///
    /// # Errors
    ///
    /// Returns `E` when the shared context rejects the measurement.
    #[inline(always)]
    fn admit<E>(&self, measurement: JsonMeasurement) -> Result<(), E>
    where
        E: Error,
    {
        if !VALUE_LIMITS {
            return Ok(());
        }
        self.context.borrow_mut().admit(measurement)
    }

    /// Enters one container only when value accounting is active.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serializer error type used to report rejected containers.
    ///
    /// # Parameters
    ///
    /// * `kind` - Container category being entered.
    /// * `depth` - Root-inclusive depth of the container.
    ///
    /// # Returns
    ///
    /// Unit after the container is admitted.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn enter_container<E>(&self, kind: JsonContainerKind, depth: usize) -> Result<(), E>
    where
        E: Error,
    {
        if !VALUE_LIMITS {
            return Ok(());
        }
        self.context.borrow_mut().enter_container(kind, depth)
    }

    /// Charges one string node and its UTF-8 payload length.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serializer error type used to report a rejected string.
    ///
    /// # Parameters
    ///
    /// * `bytes` - UTF-8 byte length of the string payload.
    ///
    /// # Returns
    ///
    /// Unit after the string is admitted.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn string<E>(&self, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::String {
            depth: self.depth,
            bytes,
        })
    }

    /// Charges one number node and its emitted textual length.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serializer error type used to report a rejected number.
    ///
    /// # Parameters
    ///
    /// * `bytes` - Length of the emitted number lexeme.
    ///
    /// # Returns
    ///
    /// Unit after the number is admitted.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn number<E>(&self, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Number {
            depth: self.depth,
            bytes,
        })
    }

    /// Charges one known-length array before its serializer is created.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serializer error type used to report a rejected array.
    ///
    /// # Parameters
    ///
    /// * `depth` - Root-inclusive depth of the array.
    /// * `items` - Number of array elements.
    ///
    /// # Returns
    ///
    /// Unit after the array is admitted.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn array<E>(&self, depth: usize, items: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Array { depth, items })
    }

    /// Charges one known-length object before its serializer is created.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serializer error type used to report a rejected object.
    ///
    /// # Parameters
    ///
    /// * `depth` - Root-inclusive depth of the object.
    /// * `entries` - Number of object entries.
    ///
    /// # Returns
    ///
    /// Unit after the object is admitted.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn object<E>(&self, depth: usize, entries: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Object { depth, entries })
    }

    /// Checks and consumes one object key's UTF-8 payload length.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serializer error type used to report a rejected key.
    ///
    /// # Parameters
    ///
    /// * `key` - Object key whose UTF-8 length is charged.
    ///
    /// # Returns
    ///
    /// Unit after the key is admitted.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn key<E>(&self, key: &str) -> Result<(), E>
    where
        E: Error,
    {
        self.admit(JsonMeasurement::Key { bytes: key.len() })
    }

    /// Creates one serializer interruption after retaining its stable kind.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serializer error type used for the interruption.
    ///
    /// # Parameters
    ///
    /// * `kind` - Stable failure category retained in the shared context.
    ///
    /// # Returns
    ///
    /// An error value containing the retained failure category.
    #[inline]
    fn serialization_error<E>(&self, kind: JsonSerializationErrorKind) -> E
    where
        E: Error,
    {
        self.context.borrow_mut().serialization_error(kind)
    }
}

macro_rules! serialize_integer {
    ($name:ident, signed $type:ty) => {
        #[doc = concat!("Charges and delegates one signed `", stringify!($type), "` JSON integer.\n\n# Parameters\n\n* `value` - Integer value to encode.\n\n# Returns\n\nThe success value produced by the underlying serializer.\n\n# Errors\n\nReturns the underlying serializer error or a budget rejection.")]
        #[inline(always)]
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            if VALUE_LIMITS {
                self.number(JsonLexemeLength::signed_integer(value.into()))?;
            }
            self.inner.$name(value)
        }
    };
    ($name:ident, unsigned $type:ty) => {
        #[doc = concat!("Charges and delegates one unsigned `", stringify!($type), "` JSON integer.\n\n# Parameters\n\n* `value` - Integer value to encode.\n\n# Returns\n\nThe success value produced by the underlying serializer.\n\n# Errors\n\nReturns the underlying serializer error or a budget rejection.")]
        #[inline(always)]
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            if VALUE_LIMITS {
                self.number(JsonLexemeLength::unsigned_integer(value.into()))?;
            }
            self.inner.$name(value)
        }
    };
}

impl<'transaction, 'budget, 'context, S, R, Q, const VALUE_LIMITS: bool> Serializer
    for JsonEncodeSerializer<'transaction, 'budget, 'context, S, R, Q, VALUE_LIMITS>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Success value produced by the underlying serializer.
    type Ok = S::Ok;
    /// Error type used by the underlying serializer for rejected events.
    type Error = S::Error;
    /// Sequence state returned while serializing a JSON array.
    type SerializeSeq =
        JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeSeq, R, Q, VALUE_LIMITS>;
    /// Tuple state returned while serializing a JSON array.
    type SerializeTuple =
        JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeTuple, R, Q, VALUE_LIMITS>;
    /// Tuple-struct state returned while serializing a JSON array.
    type SerializeTupleStruct = JsonEncodeCompound<
        'transaction,
        'budget,
        'context,
        S::SerializeTupleStruct,
        R,
        Q,
        VALUE_LIMITS,
    >;
    /// Tuple-variant state returned while serializing its outer object and
    /// array.
    type SerializeTupleVariant = JsonEncodeCompound<
        'transaction,
        'budget,
        'context,
        S::SerializeTupleVariant,
        R,
        Q,
        VALUE_LIMITS,
    >;
    /// Map state returned while serializing a JSON object.
    type SerializeMap =
        JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeMap, R, Q, VALUE_LIMITS>;
    /// Struct state returned while serializing a JSON object or recognized
    /// private protocol value.
    type SerializeStruct =
        JsonEncodeCompound<'transaction, 'budget, 'context, S::SerializeStruct, R, Q, VALUE_LIMITS>;
    /// Struct-variant state returned while serializing its outer and inner
    /// objects.
    type SerializeStructVariant = JsonEncodeCompound<
        'transaction,
        'budget,
        'context,
        S::SerializeStructVariant,
        R,
        Q,
        VALUE_LIMITS,
    >;

    /// Charges and delegates one JSON boolean.
    ///
    /// # Parameters
    ///
    /// * `value` - Boolean value to encode.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Boolean { depth: self.depth })?;
        self.inner.serialize_bool(value)
    }

    serialize_integer!(serialize_i8, signed i8);
    serialize_integer!(serialize_i16, signed i16);
    serialize_integer!(serialize_i32, signed i32);
    serialize_integer!(serialize_i64, signed i64);
    serialize_integer!(serialize_u8, unsigned u8);
    serialize_integer!(serialize_u16, unsigned u16);
    serialize_integer!(serialize_u32, unsigned u32);
    serialize_integer!(serialize_u64, unsigned u64);

    /// Charges and delegates an `i128` representable as `i64` or `u64`.
    ///
    /// # Parameters
    ///
    /// * `value` - Signed integer value to encode.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        let bytes = if let Ok(value) = i64::try_from(value) {
            JsonLexemeLength::signed_integer(value.into())
        } else if let Ok(value) = u64::try_from(value) {
            JsonLexemeLength::unsigned_integer(value.into())
        } else {
            return Err(
                self.serialization_error(JsonSerializationErrorKind::IntegerOutOfRange {
                    signedness: JsonIntegerSignedness::Signed,
                }),
            );
        };
        if VALUE_LIMITS {
            self.number(bytes)?;
        }
        self.inner.serialize_i128(value)
    }

    /// Charges and delegates a `u128` representable as `u64`.
    ///
    /// # Parameters
    ///
    /// * `value` - Unsigned integer value to encode.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        let value64 = u64::try_from(value).map_err(|_| {
            self.serialization_error(JsonSerializationErrorKind::IntegerOutOfRange {
                signedness: JsonIntegerSignedness::Unsigned,
            })
        })?;
        if VALUE_LIMITS {
            self.number(JsonLexemeLength::unsigned_integer(value64.into()))?;
        }
        self.inner.serialize_u128(value)
    }

    /// Charges and delegates one finite floating-point number.
    ///
    /// # Parameters
    ///
    /// * `value` - Floating-point value to encode; non-finite values are
    ///   rejected.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(self.serialization_error(JsonSerializationErrorKind::NonFiniteFloat));
        }
        if VALUE_LIMITS {
            self.number(JsonLexemeLength::finite_f32(value))?;
        }
        self.inner.serialize_f32(value)
    }

    /// Charges and delegates one finite floating-point number.
    ///
    /// # Parameters
    ///
    /// * `value` - Floating-point value to encode; non-finite values are
    ///   rejected.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(self.serialization_error(JsonSerializationErrorKind::NonFiniteFloat));
        }
        if VALUE_LIMITS {
            self.number(JsonLexemeLength::finite_f64(value))?;
        }
        self.inner.serialize_f64(value)
    }

    /// Charges a character as one JSON string and delegates it.
    ///
    /// # Parameters
    ///
    /// * `value` - Unicode scalar value to encode as a JSON string.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.string(value.len_utf8())?;
        self.inner.serialize_char(value)
    }

    /// Charges one JSON string before delegating it.
    ///
    /// # Parameters
    ///
    /// * `value` - String slice to encode.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.string(value.len())?;
        self.inner.serialize_str(value)
    }

    /// Charges the JSON byte-array structure before delegating it.
    ///
    /// # Parameters
    ///
    /// * `value` - Byte slice encoded using the serializer byte representation.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        if !VALUE_LIMITS {
            return self.inner.serialize_bytes(value);
        }
        self.array(self.depth, value.len())?;
        let child_depth = self.depth.saturating_add(1);
        for byte in value {
            self.admit(JsonMeasurement::Number {
                depth: child_depth,
                bytes: JsonLexemeLength::byte(*byte),
            })?;
        }
        self.inner.serialize_bytes(value)
    }

    /// Charges and delegates one JSON null.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        self.inner.serialize_none()
    }

    /// Delegates a present option transparently through the same depth.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Value serialized through the underlying serializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Present value serialized transparently at the current depth.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.depth);
        self.inner.serialize_some(&value)
    }

    /// Charges and delegates one JSON null.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        self.inner.serialize_unit()
    }

    /// Charges a unit struct as one JSON null.
    ///
    /// # Parameters
    ///
    /// * `name` - Static type name supplied by Serde.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.admit(JsonMeasurement::Null { depth: self.depth })?;
        self.inner.serialize_unit_struct(name)
    }

    /// Charges a unit variant as one JSON string.
    ///
    /// # Parameters
    ///
    /// * `name` - Static enum name.
    /// * `variant_index` - Stable index of the variant.
    /// * `variant` - Static variant name encoded as the JSON string.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.string(variant.len())?;
        self.inner
            .serialize_unit_variant(name, variant_index, variant)
    }

    /// Delegates a newtype struct transparently through the same depth.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Wrapped value type implementing Serde serialization.
    ///
    /// # Parameters
    ///
    /// * `name` - Static type name supplied by Serde.
    /// * `value` - Wrapped value serialized transparently at the current depth.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(value, self.context, self.depth);
        self.inner.serialize_newtype_struct(name, &value)
    }

    /// Charges a newtype variant's outer object and decorates its payload.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Variant payload type implementing Serde serialization.
    ///
    /// # Parameters
    ///
    /// * `name` - Static enum name.
    /// * `variant_index` - Stable index of the variant.
    /// * `variant` - Static variant name used as the outer object key.
    /// * `value` - Payload serialized one level below the variant object.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.object(self.depth, 1)?;
        self.key(variant)?;
        let value = BudgetedValue::<_, _, _, VALUE_LIMITS>::new(
            value,
            self.context,
            self.depth.saturating_add(1),
        );
        self.inner
            .serialize_newtype_variant(name, variant_index, variant, &value)
    }

    /// Charges an array before asking the inner serializer to create it.
    ///
    /// # Parameters
    ///
    /// * `len` - Optional known element count used for the initial budget
    ///   check.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let context = self.context;
        self.enter_container(JsonContainerKind::Sequence, self.depth)?;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_seq(len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            len == Some(0),
        ))
    }

    /// Charges a fixed-length JSON tuple array.
    ///
    /// # Parameters
    ///
    /// * `len` - Fixed number of tuple elements.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        let context = self.context;
        self.enter_container(JsonContainerKind::Sequence, self.depth)?;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_tuple(len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            len == 0,
        ))
    }

    /// Charges a fixed-length JSON tuple-struct array.
    ///
    /// # Parameters
    ///
    /// * `name` - Static type name supplied by Serde.
    /// * `len` - Fixed number of tuple elements.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        let context = self.context;
        self.enter_container(JsonContainerKind::Sequence, self.depth)?;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_tuple_struct(name, len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            len == 0,
        ))
    }

    /// Charges a tuple variant's outer object and nested array.
    ///
    /// # Parameters
    ///
    /// * `name` - Static enum name.
    /// * `variant_index` - Stable index of the variant.
    /// * `variant` - Static variant name used as the outer object key.
    /// * `len` - Fixed number of tuple elements.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.object(self.depth, 1)?;
        self.key(variant)?;
        let array_depth = self.depth.saturating_add(1);
        self.enter_container(JsonContainerKind::Sequence, array_depth)?;
        let context = self.context;
        let child_depth = array_depth.saturating_add(1);
        let inner = self
            .inner
            .serialize_tuple_variant(name, variant_index, variant, len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            len == 0,
        ))
    }

    /// Charges an object before asking the inner serializer to create it.
    ///
    /// # Parameters
    ///
    /// * `len` - Optional known entry count used for the initial budget check.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        let context = self.context;
        self.enter_container(JsonContainerKind::Map, self.depth)?;
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_map(len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            len == Some(0),
        ))
    }

    /// Charges a JSON object or recognizes serde_json's RawValue shape.
    ///
    /// # Parameters
    ///
    /// * `name` - Static type name supplied by Serde.
    /// * `len` - Number of struct fields.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    #[inline(always)]
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        match JsonPrivateProtocolKind::from_struct_name(name) {
            Some(kind) => {
                let context = self.context;
                let depth = self.depth;
                let state = JsonPrivateProtocolState::new(kind, len).map_err(|reason| {
                    context.borrow_mut().serialization_error(
                        JsonSerializationErrorKind::InvalidSerializerState { reason },
                    )
                })?;
                let (inner_name, inner_len) = match kind {
                    JsonPrivateProtocolKind::RawValue => (name, len),
                    JsonPrivateProtocolKind::Number => (SERDE_JSON_RAW_VALUE_TOKEN, 1),
                };
                let inner = self.inner.serialize_struct(inner_name, inner_len)?;
                Ok(JsonEncodeCompound::private(
                    inner, context, depth, kind, state,
                ))
            }
            None => {
                let context = self.context;
                self.enter_container(JsonContainerKind::Map, self.depth)?;
                let child_depth = self.depth.saturating_add(1);
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::new(
                    inner,
                    context,
                    child_depth,
                    len == 0,
                ))
            }
        }
    }

    /// Charges a struct variant's outer and inner objects.
    ///
    /// # Parameters
    ///
    /// * `name` - Static enum name.
    /// * `variant_index` - Stable index of the variant.
    /// * `variant` - Static variant name used as the outer object key.
    /// * `len` - Number of struct fields.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.object(self.depth, 1)?;
        self.key(variant)?;
        let object_depth = self.depth.saturating_add(1);
        self.enter_container(JsonContainerKind::Map, object_depth)?;
        let context = self.context;
        let child_depth = object_depth.saturating_add(1);
        let inner = self
            .inner
            .serialize_struct_variant(name, variant_index, variant, len)?;
        Ok(JsonEncodeCompound::new(
            inner,
            context,
            child_depth,
            len == 0,
        ))
    }

    /// Formats one display value once, checks it as a string, and emits it.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Display type whose formatted representation is emitted.
    ///
    /// # Parameters
    ///
    /// * `value` - Display value formatted once and emitted as a string.
    ///
    /// # Returns
    ///
    /// The success value produced by the underlying serializer.
    ///
    /// # Errors
    ///
    /// Returns an underlying serializer error or a JSON budget rejection.
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        let text = JsonEncodeContext::collect_display::<S::Error, _>(
            self.context,
            value,
            DisplayBudgetKind::String,
            self.depth,
        )?;
        self.inner.serialize_str(&text)
    }

    /// Preserves the underlying serializer's readability contract.
    ///
    /// # Returns
    ///
    /// `true` when the underlying serializer uses a human-readable
    /// representation.
    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
