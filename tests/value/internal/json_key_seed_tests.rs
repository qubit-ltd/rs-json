// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests map-key admission through the public JSON value seed.

use std::cell::Cell;
use std::rc::Rc;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::AccountingJsonValueSeed;
use serde::Deserializer as SerdeDeserializer;
use serde::de::DeserializeSeed;
use serde::de::Error as DeError;
use serde::de::MapAccess;
use serde::de::Visitor;
use serde::de::value;
use serde::forward_to_deserialize_any;
use serde_json::Deserializer;

/// Verifies a second object key is rejected before it is materialized.
#[test]
fn test_json_key_seed_checks_prospective_object_entry() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(StructureLimits::builder().map_entries_limit(ResourceLimit::new(JsonResource::MapEntries, 1)))
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"a":null,"b":null}"#);

    AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err("the second key must exceed the one-entry map limit");
    let error = transaction
        .commit()
        .expect_err("failed admission must poison the transaction");
    assert_eq!(error.resource(), &JsonResource::MapEntries);
}

#[test]
fn test_json_key_seed_admits_borrowed_key_before_requesting_owned_text() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_key_bytes(8)
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let next_value_calls = Rc::new(Cell::new(0));
    let key_deserialize_calls = Rc::new(Cell::new(0));
    let deserializer = OneEntryMapDeserializer {
        key: "123456789",
        next_value_calls: Rc::clone(&next_value_calls),
        key_deserialize_calls: Rc::clone(&key_deserialize_calls),
    };

    let error = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(deserializer)
        .expect_err("the borrowed key must be rejected before requesting its value");
    assert!(error.to_string().contains("KeyBytes"), "unexpected error: {error}");
    assert_eq!(next_value_calls.get(), 0);
    assert_eq!(key_deserialize_calls.get(), 1);
    let error = transaction
        .commit()
        .expect_err("key rejection must poison the transaction");
    assert_eq!(error.resource(), &JsonResource::KeyBytes);
}

#[test]
fn test_json_key_seed_checks_map_entries_before_key_deserialization() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(StructureLimits::builder().map_entries_limit(ResourceLimit::new(JsonResource::MapEntries, 0)))
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let next_value_calls = Rc::new(Cell::new(0));
    let key_deserialize_calls = Rc::new(Cell::new(0));
    let deserializer = OneEntryMapDeserializer {
        key: "a",
        next_value_calls: Rc::clone(&next_value_calls),
        key_deserialize_calls: Rc::clone(&key_deserialize_calls),
    };

    AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(deserializer)
        .expect_err("the zero-entry map limit must reject before decoding its key");
    assert_eq!(key_deserialize_calls.get(), 0);
    assert_eq!(next_value_calls.get(), 0);
    assert_eq!(transaction.commit().unwrap_err().resource(), &JsonResource::MapEntries);
}

#[test]
fn test_json_key_seed_accepts_key_at_byte_limit_and_rejects_one_over() {
    for (input, maximum, expected_resource) in [
        (br#"{"12345678":null}"#.as_slice(), 8, None),
        (br#"{"123456789":null}"#.as_slice(), 8, Some(JsonResource::KeyBytes)),
    ] {
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_key_bytes(maximum)
            .build();
        let mut budget = limits.budget();
        let mut transaction = budget.transaction();
        let mut deserializer = Deserializer::from_slice(input);
        let result = AccountingJsonValueSeed::new(&mut transaction).deserialize(&mut deserializer);
        match expected_resource {
            Some(resource) => {
                result.expect_err("a key one byte over the limit must fail");
                assert_eq!(transaction.commit().unwrap_err().resource(), &resource);
            }
            None => {
                result.expect("a key at the limit must be admitted");
                transaction.commit().expect("the admitted key must commit");
            }
        }
    }
}

#[test]
fn test_json_key_seed_counts_decoded_bytes_of_escaped_key_once() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_key_bytes(3)
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"a\nb":null}"#);

    let value = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect("the decoded key contains three bytes and is admitted once");
    assert_eq!(value.as_object().unwrap().keys().next().unwrap(), "a\nb");
    transaction.commit().expect("one decoded three-byte key must fit");
}

#[test]
fn test_json_key_seed_adopts_owned_key_without_double_accounting() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_key_bytes(3)
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();

    let value = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(OwnedOneEntryMapDeserializer { key_pending: true })
        .expect("an owned three-byte key must fit exactly once");
    assert_eq!(value.as_object().unwrap().keys().next().unwrap(), "key");
    transaction
        .commit()
        .expect("the map visitor must not charge the already-admitted key again");
}

#[test]
fn test_json_key_seed_preserves_payload_error_priority() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_key_bytes(1)
        .max_payload_bytes(1)
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"a":"xx","123456789":null}"#);

    AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err("the first value must exceed payload before the later oversized key");
    assert_eq!(
        transaction.commit().unwrap_err().resource(),
        &JsonResource::PayloadBytes
    );
}

#[test]
fn test_json_key_seed_accounts_payload_for_duplicate_entries() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_payload_bytes(3)
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"dup":"ab","dup":"cd"}"#);

    AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err("payload from both duplicate entries must be accounted");
    assert_eq!(
        transaction.commit().unwrap_err().resource(),
        &JsonResource::PayloadBytes
    );
}

#[test]
fn test_json_key_seed_rejects_non_string_key_events() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();

    AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(value::MapDeserializer::<_, value::Error>::new([(1_u8, ())].into_iter()))
        .expect_err("numeric keys must remain invalid JSON object keys");
}

struct OneEntryMapDeserializer<'de> {
    key: &'de str,
    next_value_calls: Rc<Cell<usize>>,
    key_deserialize_calls: Rc<Cell<usize>>,
}

impl<'de> SerdeDeserializer<'de> for OneEntryMapDeserializer<'de> {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(OneEntryMapAccess {
            key: Some(self.key),
            next_value_calls: self.next_value_calls,
            key_deserialize_calls: self.key_deserialize_calls,
        })
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
}

struct OneEntryMapAccess<'de> {
    key: Option<&'de str>,
    next_value_calls: Rc<Cell<usize>>,
    key_deserialize_calls: Rc<Cell<usize>>,
}

impl<'de> MapAccess<'de> for OneEntryMapAccess<'de> {
    type Error = value::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        self.key
            .take()
            .map(|key| {
                seed.deserialize(BorrowedOnlyKeyDeserializer {
                    value: key,
                    calls: Rc::clone(&self.key_deserialize_calls),
                })
            })
            .transpose()
    }

    fn next_value_seed<V>(&mut self, _seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        self.next_value_calls.set(self.next_value_calls.get() + 1);
        Err(value::Error::custom("unexpected next_value"))
    }
}

struct BorrowedOnlyKeyDeserializer<'de> {
    value: &'de str,
    calls: Rc<Cell<usize>>,
}

impl<'de> SerdeDeserializer<'de> for BorrowedOnlyKeyDeserializer<'de> {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.calls.set(self.calls.get() + 1);
        visitor.visit_borrowed_str(self.value)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.calls.set(self.calls.get() + 1);
        visitor.visit_borrowed_str(self.value)
    }

    fn deserialize_string<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.calls.set(self.calls.get() + 1);
        Err(value::Error::custom("owned string requested"))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum
        identifier ignored_any
    }
}

struct OwnedOneEntryMapDeserializer {
    key_pending: bool,
}

impl<'de> SerdeDeserializer<'de> for OwnedOneEntryMapDeserializer {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(OwnedOneEntryMapAccess {
            key_pending: self.key_pending,
        })
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct
        map struct enum identifier ignored_any
    }
}

struct OwnedOneEntryMapAccess {
    key_pending: bool,
}

impl<'de> MapAccess<'de> for OwnedOneEntryMapAccess {
    type Error = value::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if std::mem::replace(&mut self.key_pending, false) {
            seed.deserialize(OwnedKeyDeserializer).map(Some)
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(value::UnitDeserializer::<value::Error>::new())
    }
}

struct OwnedKeyDeserializer;

impl<'de> SerdeDeserializer<'de> for OwnedKeyDeserializer {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(String::from("key"))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(String::from("key"))
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(String::from("key"))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum
        identifier ignored_any
    }
}
