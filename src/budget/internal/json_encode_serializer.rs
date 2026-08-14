// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Single-pass Serde JSON encoder with online budget checks.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types
// qubit-style: allow explicit-imports

use std::cell::RefCell;
use std::fmt;
use std::fmt::Display;
use std::fmt::Write as _;
use std::rc::Rc;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error;

use super::super::JsonSerdeError;
use super::super::JsonValueBudget;
use super::JsonLexicalPreflight;
use super::json_encode_compound::BudgetedValue;
use super::json_encode_compound::JsonEncodeCompound;
use super::json_output_buffer::JsonOutputAccounting;
use super::private_struct_kind::PrivateStructKind;
use super::serde_json_compat::classify_private_struct;

/// Mutable state shared by every decorator in one serialization traversal.
pub(super) struct JsonEncodeContext<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Caller-owned budget charged by the traversal.
    pub(super) budget: &'a mut JsonValueBudget<R, Q>,

    /// Live output accounting shared with the byte buffer.
    output: Rc<RefCell<JsonOutputAccounting<'a, R, Q>>>,
}

impl<R, Q> JsonEncodeContext<'_, R, Q>
where
    Q: ResourceQuantity,
{
    /// Records one failed check before converting it into a Serde error.
    pub(super) fn record<E>(
        &mut self,
        result: Result<(), MeasuredBudgetError<R, Q>>,
    ) -> Result<(), E>
    where
        E: Error,
    {
        result.map_err(|error| {
            self.output.borrow_mut().record_violation(error);
            E::custom("JSON resource budget exceeded")
        })
    }

    /// Checks and charges one raw JSON fragment before it reaches the writer.
    ///
    /// The fragment length is a safe lower bound for the complete output size.
    /// Structural traversal starts at `depth`, the fragment's root-inclusive
    /// position in the final document.
    fn preflight_raw<E>(&mut self, value: &str, depth: usize) -> Result<(), E>
    where
        E: Error,
        R: Clone,
    {
        let output = self.output.borrow().check_available(value.len());
        self.record(output)?;
        match JsonLexicalPreflight::at_depth(self.budget, depth)
            .inspect(value.as_bytes())
        {
            Ok(()) => Ok(()),
            Err(JsonSerdeError::Budget(error)) => {
                self.record(Err(error.into()))
            }
            Err(JsonSerdeError::Quantity { resource, source }) => {
                self.record(Err(MeasuredBudgetError::Quantity {
                    resource,
                    source,
                }))
            }
            Err(JsonSerdeError::Json(_) | JsonSerdeError::Io(_)) => {
                Err(E::custom("invalid raw JSON value"))
            }
            Err(JsonSerdeError::Syntax(_)) => {
                Err(E::custom("invalid raw JSON value"))
            }
        }
    }
}

/// Resource checked while a `Display` implementation emits text chunks.
#[derive(Clone, Copy)]
enum DisplayBudgetKind {
    /// Ordinary JSON string payload.
    String,

    /// JSON object key text.
    Key,

    /// Arbitrary-precision JSON number text.
    Number,

    /// Raw JSON source, bounded by the complete output limit while collected.
    RawOutput,
}

/// Bounded string sink used by Serde `collect_str` hooks.
///
/// A fallible `Display` wrapper cannot safely be delegated to serde_json:
/// its streaming string adapter assumes every formatting error came from its
/// writer, while private Number/RawValue emitters may call `to_string` first.
/// This collector therefore owns the failure boundary and caps allocation
/// before passing an already bounded `str` to the inner serializer.
struct BudgetedDisplayCollector<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Text accepted by the relevant budget so far.
    text: String,

    /// Shared traversal state retaining typed budget errors.
    context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,

    /// Resource semantics applied to the collected text.
    kind: DisplayBudgetKind,
}

impl<'a, R, Q> BudgetedDisplayCollector<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an empty collector for one resource kind.
    fn new(
        context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,
        kind: DisplayBudgetKind,
    ) -> Self {
        Self {
            text: String::new(),
            context,
            kind,
        }
    }
}

impl<R, Q> fmt::Write for BudgetedDisplayCollector<'_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Checks the cumulative formatted length before growing the string.
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next =
            self.text.len().checked_add(value.len()).ok_or(fmt::Error)?;
        let output_result =
            self.context.borrow().output.borrow().check_available(next);
        self.context
            .borrow_mut()
            .record::<fmt::Error>(output_result)?;
        let point_result = {
            let context = self.context.borrow();
            match self.kind {
                DisplayBudgetKind::String => {
                    context.budget.check_string_bytes_usize(next)
                }
                DisplayBudgetKind::Key => {
                    context.budget.check_key_bytes_usize(next)
                }
                DisplayBudgetKind::Number => {
                    context.budget.check_number_bytes_usize(next)
                }
                DisplayBudgetKind::RawOutput => Ok(()),
            }
        };
        self.context
            .borrow_mut()
            .record::<fmt::Error>(point_result)?;
        self.text.push_str(value);
        Ok(())
    }
}

/// Formats one display value into a budgeted collector.
fn collect_display<'a, E, T, R, Q>(
    value: &T,
    context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,
    kind: DisplayBudgetKind,
) -> Result<String, E>
where
    E: Error,
    T: Display + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    let mut collector = BudgetedDisplayCollector::new(context, kind);
    write!(&mut collector, "{value}")
        .map_err(|_| E::custom("JSON display text budget exceeded"))?;
    let text = collector.text;
    let payload_result = {
        let mut context = collector.context.borrow_mut();
        match kind {
            DisplayBudgetKind::String => {
                context.budget.consume_string_bytes_usize(text.len())
            }
            DisplayBudgetKind::Key => {
                context.budget.consume_key_bytes_usize(text.len())
            }
            DisplayBudgetKind::Number => {
                context.budget.consume_number_bytes_usize(text.len())
            }
            DisplayBudgetKind::RawOutput => Ok(()),
        }
    };
    collector.context.borrow_mut().record::<E>(payload_result)?;
    Ok(text)
}

/// Decorates one Serde serializer with eager JSON budget checks.
pub(in crate::budget) struct JsonEncodeSerializer<'a, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// Underlying serializer that emits JSON events.
    inner: S,

    /// Shared mutable state for this traversal.
    context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,

    /// Root-inclusive depth assigned to the current value.
    depth: usize,
}

impl<'a, S, R, Q> JsonEncodeSerializer<'a, S, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a root decorator at depth one.
    ///
    /// # Parameters
    ///
    /// * `inner` - Underlying serializer that emits the JSON document.
    /// * `budget` - Caller-owned budget charged before every delegated event.
    /// * `output` - Live output accounting shared with the byte buffer.
    ///
    /// # Returns
    ///
    /// A decorator positioned at the root JSON value.
    pub(in crate::budget) fn new(
        inner: S,
        budget: &'a mut JsonValueBudget<R, Q>,
        output: Rc<RefCell<JsonOutputAccounting<'a, R, Q>>>,
    ) -> Self {
        Self {
            inner,
            context: Rc::new(RefCell::new(JsonEncodeContext {
                budget,
                output,
            })),
            depth: 1,
        }
    }

    /// Creates a nested decorator sharing an existing traversal context.
    pub(super) const fn with_context(
        inner: S,
        context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,
        depth: usize,
    ) -> Self {
        Self {
            inner,
            context,
            depth,
        }
    }

    /// Records one budget result after all temporary context borrows end.
    fn record<E>(
        &self,
        result: Result<(), MeasuredBudgetError<R, Q>>,
    ) -> Result<(), E>
    where
        E: Error,
    {
        self.context.borrow_mut().record(result)
    }

    /// Charges one scalar or container node at the current depth.
    fn node<E>(&self) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_node_usize(self.depth);
        self.record(result)
    }

    /// Charges one string node and its UTF-8 payload length.
    fn string<E>(&self, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.node()?;
        let result = self
            .context
            .borrow_mut()
            .budget
            .consume_string_bytes_usize(bytes);
        self.record(result)
    }

    /// Charges one number node and its emitted textual length.
    fn number<E>(&self, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        self.node()?;
        let result = self
            .context
            .borrow_mut()
            .budget
            .consume_number_bytes_usize(bytes);
        self.record(result)
    }

    /// Charges one known-length array before its serializer is created.
    fn array<E>(&self, depth: usize, items: usize) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_array_usize(depth, items);
        self.record(result)
    }

    /// Charges one known-length object before its serializer is created.
    fn object<E>(&self, depth: usize, entries: usize) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_object_usize(depth, entries);
        self.record(result)
    }

    /// Checks and consumes one object key's UTF-8 payload length.
    fn key<E>(&self, key: &str) -> Result<(), E>
    where
        E: Error,
    {
        let result = self
            .context
            .borrow_mut()
            .budget
            .consume_key_bytes_usize(key.len());
        self.record(result)
    }
}

macro_rules! serialize_integer {
    ($name:ident, $type:ty) => {
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.number(value.to_string().len())?;
            self.inner.$name(value)
        }
    };
}

impl<'a, S, R, Q> Serializer for JsonEncodeSerializer<'a, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = JsonEncodeCompound<'a, S::SerializeSeq, R, Q>;
    type SerializeTuple = JsonEncodeCompound<'a, S::SerializeTuple, R, Q>;
    type SerializeTupleStruct =
        JsonEncodeCompound<'a, S::SerializeTupleStruct, R, Q>;
    type SerializeTupleVariant =
        JsonEncodeCompound<'a, S::SerializeTupleVariant, R, Q>;
    type SerializeMap = JsonEncodeCompound<'a, S::SerializeMap, R, Q>;
    type SerializeStruct = JsonEncodeCompound<'a, S::SerializeStruct, R, Q>;
    type SerializeStructVariant =
        JsonEncodeCompound<'a, S::SerializeStructVariant, R, Q>;

    /// Charges and delegates one JSON boolean.
    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.node()?;
        self.inner.serialize_bool(value)
    }

    serialize_integer!(serialize_i8, i8);
    serialize_integer!(serialize_i16, i16);
    serialize_integer!(serialize_i32, i32);
    serialize_integer!(serialize_i64, i64);
    serialize_integer!(serialize_i128, i128);
    serialize_integer!(serialize_u8, u8);
    serialize_integer!(serialize_u16, u16);
    serialize_integer!(serialize_u32, u32);
    serialize_integer!(serialize_u64, u64);
    serialize_integer!(serialize_u128, u128);

    /// Charges and delegates one floating-point number or JSON null.
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            let bytes = serde_json::to_string(&value)
                .map_err(S::Error::custom)?
                .len();
            self.number(bytes)?;
        } else {
            self.node()?;
        }
        self.inner.serialize_f32(value)
    }

    /// Charges and delegates one floating-point number or JSON null.
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            let bytes = serde_json::to_string(&value)
                .map_err(S::Error::custom)?
                .len();
            self.number(bytes)?;
        } else {
            self.node()?;
        }
        self.inner.serialize_f64(value)
    }

    /// Charges a character as one JSON string and delegates it.
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.string(value.len_utf8())?;
        self.inner.serialize_char(value)
    }

    /// Charges one JSON string before delegating it.
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.string(value.len())?;
        self.inner.serialize_str(value)
    }

    /// Charges the JSON byte-array structure before delegating it.
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.array(self.depth, value.len())?;
        let child_depth = self.depth.saturating_add(1);
        for byte in value {
            let result = self
                .context
                .borrow_mut()
                .budget
                .enter_node_usize(child_depth);
            self.record(result)?;
            let result = self
                .context
                .borrow_mut()
                .budget
                .consume_number_bytes_usize(byte.to_string().len());
            self.record(result)?;
        }
        self.inner.serialize_bytes(value)
    }

    /// Charges and delegates one JSON null.
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.node()?;
        self.inner.serialize_none()
    }

    /// Delegates a present option transparently through the same depth.
    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value =
            BudgetedValue::new(value, Rc::clone(&self.context), self.depth);
        self.inner.serialize_some(&value)
    }

    /// Charges and delegates one JSON null.
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.node()?;
        self.inner.serialize_unit()
    }

    /// Charges a unit struct as one JSON null.
    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.node()?;
        self.inner.serialize_unit_struct(name)
    }

    /// Charges a unit variant as one JSON string.
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
    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value =
            BudgetedValue::new(value, Rc::clone(&self.context), self.depth);
        self.inner.serialize_newtype_struct(name, &value)
    }

    /// Charges a newtype variant's outer object and decorates its payload.
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
        let value = BudgetedValue::new(
            value,
            Rc::clone(&self.context),
            self.depth.saturating_add(1),
        );
        self.inner.serialize_newtype_variant(
            name,
            variant_index,
            variant,
            &value,
        )
    }

    /// Charges an array before asking the inner serializer to create it.
    fn serialize_seq(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        self.node()?;
        let context = Rc::clone(&self.context);
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_seq(len)?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges a fixed-length JSON tuple array.
    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        self.node()?;
        let context = Rc::clone(&self.context);
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_tuple(len)?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges a fixed-length JSON tuple-struct array.
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.node()?;
        let context = Rc::clone(&self.context);
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_tuple_struct(name, len)?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges a tuple variant's outer object and nested array.
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
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_node_usize(array_depth);
        self.record(result)?;
        let context = Rc::clone(&self.context);
        let child_depth = array_depth.saturating_add(1);
        let inner = self.inner.serialize_tuple_variant(
            name,
            variant_index,
            variant,
            len,
        )?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges an object before asking the inner serializer to create it.
    fn serialize_map(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeMap, Self::Error> {
        if let Some(entries) = len {
            let result = self
                .context
                .borrow()
                .budget
                .check_map_entries_usize(entries);
            self.record(result)?;
        }
        self.node()?;
        let context = Rc::clone(&self.context);
        let child_depth = self.depth.saturating_add(1);
        let inner = self.inner.serialize_map(len)?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Charges a JSON object or recognizes serde_json's private number shape.
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        match classify_private_struct(name) {
            Some(PrivateStructKind::Number) => {
                self.node()?;
                let context = Rc::clone(&self.context);
                let depth = self.depth;
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::number(inner, context, depth))
            }
            Some(PrivateStructKind::RawValue) => {
                let context = Rc::clone(&self.context);
                let depth = self.depth;
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::raw_value(inner, context, depth))
            }
            None => {
                self.node()?;
                let context = Rc::clone(&self.context);
                let child_depth = self.depth.saturating_add(1);
                let inner = self.inner.serialize_struct(name, len)?;
                Ok(JsonEncodeCompound::new(inner, context, child_depth))
            }
        }
    }

    /// Charges a struct variant's outer and inner objects.
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
        let result = self
            .context
            .borrow_mut()
            .budget
            .enter_node_usize(object_depth);
        self.record(result)?;
        let context = Rc::clone(&self.context);
        let child_depth = object_depth.saturating_add(1);
        let inner = self.inner.serialize_struct_variant(
            name,
            variant_index,
            variant,
            len,
        )?;
        Ok(JsonEncodeCompound::new(inner, context, child_depth))
    }

    /// Formats one display value once, checks it as a string, and emits it.
    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        self.node()?;
        let text = collect_display::<S::Error, _, _, Q>(
            value,
            Rc::clone(&self.context),
            DisplayBudgetKind::String,
        )?;
        self.inner.serialize_str(&text)
    }

    /// Preserves the underlying serializer's readability contract.
    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

/// Wraps a map key so it is traversed once through a key-aware decorator.
pub(super) struct BudgetedKey<'a, 'budget, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Original map key.
    value: &'a T,

    /// Shared traversal context.
    context: Rc<RefCell<JsonEncodeContext<'budget, R, Q>>>,
}

impl<'a, 'budget, T, R, Q> BudgetedKey<'a, 'budget, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Creates a key wrapper bound to the shared traversal context.
    pub(super) const fn new(
        value: &'a T,
        context: Rc<RefCell<JsonEncodeContext<'budget, R, Q>>>,
    ) -> Self {
        Self { value, context }
    }
}

impl<T, R, Q> Serialize for BudgetedKey<'_, '_, T, R, Q>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Serializes the original key once through a key-aware decorator.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(JsonKeyBudgetSerializer {
            inner: serializer,
            context: Rc::clone(&self.context),
        })
    }
}

/// Decorates serde_json's map-key serializer with key-byte checks.
struct JsonKeyBudgetSerializer<'a, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// Underlying map-key serializer.
    inner: S,

    /// Shared traversal context.
    context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,
}

impl<S, R, Q> JsonKeyBudgetSerializer<'_, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Checks and consumes one emitted key length, retaining any original
    /// error.
    fn check(&self, bytes: usize) -> Result<(), S::Error> {
        let result = self
            .context
            .borrow_mut()
            .budget
            .consume_key_bytes_usize(bytes);
        self.context.borrow_mut().record(result)
    }
}

macro_rules! serialize_key_number {
    ($name:ident, $type:ty) => {
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.check(value.to_string().len())?;
            self.inner.$name(value)
        }
    };
}

impl<'a, S, R, Q> Serializer for JsonKeyBudgetSerializer<'a, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = S::SerializeSeq;
    type SerializeTuple = S::SerializeTuple;
    type SerializeTupleStruct = S::SerializeTupleStruct;
    type SerializeTupleVariant = S::SerializeTupleVariant;
    type SerializeMap = S::SerializeMap;
    type SerializeStruct = S::SerializeStruct;
    type SerializeStructVariant = S::SerializeStructVariant;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.check(if value { 4 } else { 5 })?;
        self.inner.serialize_bool(value)
    }

    serialize_key_number!(serialize_i8, i8);
    serialize_key_number!(serialize_i16, i16);
    serialize_key_number!(serialize_i32, i32);
    serialize_key_number!(serialize_i64, i64);
    serialize_key_number!(serialize_i128, i128);
    serialize_key_number!(serialize_u8, u8);
    serialize_key_number!(serialize_u16, u16);
    serialize_key_number!(serialize_u32, u32);
    serialize_key_number!(serialize_u64, u64);
    serialize_key_number!(serialize_u128, u128);

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            let bytes = serde_json::to_string(&value)
                .map_err(S::Error::custom)?
                .len();
            self.check(bytes)?;
        }
        self.inner.serialize_f32(value)
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
            let bytes = serde_json::to_string(&value)
                .map_err(S::Error::custom)?
                .len();
            self.check(bytes)?;
        }
        self.inner.serialize_f64(value)
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.check(value.len_utf8())?;
        self.inner.serialize_char(value)
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.check(value.len())?;
        self.inner.serialize_str(value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_bytes(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_none()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit()
    }

    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.check(variant.len())?;
        self.inner
            .serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

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
        self.inner.serialize_newtype_variant(
            name,
            variant_index,
            variant,
            value,
        )
    }

    fn serialize_seq(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        self.inner.serialize_seq(len)
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        self.inner.serialize_tuple(len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.inner.serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.inner
            .serialize_tuple_variant(name, variant_index, variant, len)
    }

    fn serialize_map(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeMap, Self::Error> {
        self.inner.serialize_map(len)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.inner.serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.inner
            .serialize_struct_variant(name, variant_index, variant, len)
    }

    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        let text = collect_display::<S::Error, _, _, Q>(
            value,
            Rc::clone(&self.context),
            DisplayBudgetKind::Key,
        )?;
        self.inner.serialize_str(&text)
    }

    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

/// Wraps a serde_json private string payload.
pub(super) struct BudgetedPrivateValue<'a, 'budget, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Private string payload supplied by serde_json.
    value: &'a T,

    /// Shared traversal context.
    context: Rc<RefCell<JsonEncodeContext<'budget, R, Q>>>,

    /// Budget semantics represented by the private string payload.
    kind: PrivateTextKind,
}

impl<'a, 'budget, T, R, Q> BudgetedPrivateValue<'a, 'budget, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Creates a private arbitrary-precision number payload wrapper.
    pub(super) const fn number(
        value: &'a T,
        context: Rc<RefCell<JsonEncodeContext<'budget, R, Q>>>,
    ) -> Self {
        Self {
            value,
            context,
            kind: PrivateTextKind::Number,
        }
    }

    /// Creates a private raw JSON payload wrapper at its final depth.
    pub(super) const fn raw_value(
        value: &'a T,
        context: Rc<RefCell<JsonEncodeContext<'budget, R, Q>>>,
        depth: usize,
    ) -> Self {
        Self {
            value,
            context,
            kind: PrivateTextKind::RawValue { depth },
        }
    }
}

impl<T, R, Q> Serialize for BudgetedPrivateValue<'_, '_, T, R, Q>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Traverses the private payload once through its text decorator.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(JsonPrivateTextSerializer {
            inner: serializer,
            context: Rc::clone(&self.context),
            kind: self.kind,
        })
    }
}

/// Budget semantics for a serde_json private string payload.
#[derive(Clone, Copy)]
enum PrivateTextKind {
    /// Arbitrary-precision number text.
    Number,

    /// Raw JSON fragment rooted at the specified final depth.
    RawValue { depth: usize },
}

/// Checks the string token emitted by a serde_json private serializer.
struct JsonPrivateTextSerializer<'a, S, R, Q>
where
    Q: ResourceQuantity,
{
    /// serde_json private string emitter.
    inner: S,

    /// Shared traversal context.
    context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,

    /// Budget semantics represented by the emitted text.
    kind: PrivateTextKind,
}

macro_rules! delegate_number_method {
    ($name:ident, $type:ty) => {
        fn $name(self, value: $type) -> Result<Self::Ok, Self::Error> {
            self.inner.$name(value)
        }
    };
}

impl<'a, S, R, Q> Serializer for JsonPrivateTextSerializer<'a, S, R, Q>
where
    S: Serializer,
    R: Clone,
    Q: ResourceQuantity,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = S::SerializeSeq;
    type SerializeTuple = S::SerializeTuple;
    type SerializeTupleStruct = S::SerializeTupleStruct;
    type SerializeTupleVariant = S::SerializeTupleVariant;
    type SerializeMap = S::SerializeMap;
    type SerializeStruct = S::SerializeStruct;
    type SerializeStructVariant = S::SerializeStructVariant;

    delegate_number_method!(serialize_bool, bool);
    delegate_number_method!(serialize_i8, i8);
    delegate_number_method!(serialize_i16, i16);
    delegate_number_method!(serialize_i32, i32);
    delegate_number_method!(serialize_i64, i64);
    delegate_number_method!(serialize_i128, i128);
    delegate_number_method!(serialize_u8, u8);
    delegate_number_method!(serialize_u16, u16);
    delegate_number_method!(serialize_u32, u32);
    delegate_number_method!(serialize_u64, u64);
    delegate_number_method!(serialize_u128, u128);
    delegate_number_method!(serialize_f32, f32);
    delegate_number_method!(serialize_f64, f64);
    delegate_number_method!(serialize_char, char);

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        match self.kind {
            PrivateTextKind::Number => {
                let number = self
                    .context
                    .borrow_mut()
                    .budget
                    .consume_number_bytes_usize(value.len());
                self.context.borrow_mut().record(number)?;
            }
            PrivateTextKind::RawValue { depth } => {
                self.context.borrow_mut().preflight_raw(value, depth)?;
            }
        }
        self.inner.serialize_str(value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_bytes(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_none()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.inner.serialize_some(value)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit()
    }

    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.inner
            .serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.inner.serialize_newtype_struct(name, value)
    }

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
        self.inner.serialize_newtype_variant(
            name,
            variant_index,
            variant,
            value,
        )
    }

    fn serialize_seq(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Self::Error> {
        self.inner.serialize_seq(len)
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Self::Error> {
        self.inner.serialize_tuple(len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.inner.serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.inner
            .serialize_tuple_variant(name, variant_index, variant, len)
    }

    fn serialize_map(
        self,
        len: Option<usize>,
    ) -> Result<Self::SerializeMap, Self::Error> {
        self.inner.serialize_map(len)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.inner.serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.inner
            .serialize_struct_variant(name, variant_index, variant, len)
    }

    fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Display + ?Sized,
    {
        let budget_kind = match self.kind {
            PrivateTextKind::Number => DisplayBudgetKind::Number,
            PrivateTextKind::RawValue { .. } => DisplayBudgetKind::RawOutput,
        };
        let text = collect_display::<S::Error, _, _, Q>(
            value,
            Rc::clone(&self.context),
            budget_kind,
        )?;
        match self.kind {
            PrivateTextKind::Number => {}
            PrivateTextKind::RawValue { depth } => {
                self.context.borrow_mut().preflight_raw(&text, depth)?;
            }
        }
        self.inner.serialize_str(&text)
    }

    #[inline(always)]
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
