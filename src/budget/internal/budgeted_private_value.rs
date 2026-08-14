// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde_json private Number and RawValue serializer adapters.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types
// qubit-style: allow explicit-imports

use std::cell::RefCell;
use std::fmt::Display;
use std::rc::Rc;

use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;

use super::display_budget_kind::DisplayBudgetKind;
use super::json_encode_context::JsonEncodeContext;
use super::json_encode_context::collect_display;

/// Wraps a serde_json private string payload with budget accounting.
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
