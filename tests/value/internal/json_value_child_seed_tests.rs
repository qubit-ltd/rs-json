// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests nested child admission through the public JSON value seed.

use std::cell::Cell;
use std::rc::Rc;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::AccountingJsonValueSeed;
use serde::de::DeserializeSeed;
use serde::de::IntoDeserializer;
use serde::de::Visitor;
use serde::de::value;
use serde::forward_to_deserialize_any;
use serde::Deserializer as SerdeDeserializer;
use serde_json::json;
use serde_json::Deserializer;

struct TrackedUnit {
    calls: Rc<Cell<usize>>,
}

impl<'de> IntoDeserializer<'de, value::Error> for TrackedUnit {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> SerdeDeserializer<'de> for TrackedUnit {
    type Error = value::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.calls.set(self.calls.get() + 1);
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

/// Verifies an excess array child is rejected before materialization.
#[test]
fn test_json_value_child_seed_checks_prospective_array_item() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(
            StructureLimits::builder().sequence_items_limit(ResourceLimit::new(JsonResource::SequenceItems, 1)),
        )
        .build();
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"[null,null]"#);

    AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect_err("the second array item must exceed the one-item sequence limit");
    let error = transaction
        .commit()
        .expect_err("failed admission must poison the transaction");
    assert_eq!(error.resource(), &JsonResource::SequenceItems);
}

#[test]
fn test_json_value_child_seed_rejects_before_child_deserialization() {
    for maximum in [0_usize, 1, 2] {
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .structure_limits(StructureLimits::builder().sequence_items_limit(
                ResourceLimit::new(JsonResource::SequenceItems, maximum),
            ))
            .build();
        let mut budget = limits.budget();
        let mut transaction = budget.transaction();
        let first = Rc::new(Cell::new(0));
        let second = Rc::new(Cell::new(0));
        let children = [
            TrackedUnit {
                calls: Rc::clone(&first),
            },
            TrackedUnit {
                calls: Rc::clone(&second),
            },
        ];
        let deserializer = value::SeqDeserializer::<_, value::Error>::new(children.into_iter());
        let result = AccountingJsonValueSeed::new(&mut transaction).deserialize(deserializer);

        assert_eq!(first.get(), usize::from(maximum > 0));
        assert_eq!(second.get(), usize::from(maximum > 1));
        if maximum < 2 {
            result.expect_err("excess children must be rejected");
            assert_eq!(
                transaction.commit().unwrap_err().resource(),
                &JsonResource::SequenceItems,
            );
        } else {
            assert_eq!(result.unwrap(), json!([null, null]));
            transaction
                .commit()
                .expect("both admitted children must commit");
        }
    }
}
