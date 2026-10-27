use crate::lexical::JsonLexicalEvent;
use crate::lexical::JsonNumberModel;
use crate::value::SafeJsonValue;
use crate::value::internal::LexicalJsonValueBuilder;
use serde::Deserialize;
use serde_json::Value;

#[test]
fn test_owner_deep_lifecycle_in_small_stack() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut raw = Value::Null;
            for _ in 0..32_768 {
                raw = Value::Array(vec![raw]);
            }
            let owner = SafeJsonValue::new(raw);
            let cloned = owner.clone();
            assert!(owner == cloned);
            drop(cloned);
            drop(owner);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn test_owner_handles_object_chain_and_wide_array() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut object_tree = Value::Null;
            for index in 0..16_384 {
                let mut object = serde_json::Map::new();
                object.insert(format!("k{index}"), object_tree);
                object_tree = Value::Object(object);
            }
            let owner = SafeJsonValue::new(object_tree);
            let wide = SafeJsonValue::new(Value::Array(vec![Value::Null; 32_768]));
            drop(owner);
            drop(wide);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn test_owner_debug_propagates_writer_failure() {
    struct FailingSink;
    impl std::fmt::Write for FailingSink {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    let owner = SafeJsonValue::new(serde_json::json!([1, {"x": true}]));
    assert!(std::fmt::write(&mut FailingSink, format_args!("{owner:?}")).is_err());
}

#[test]
fn test_owner_drop_uses_allocation_free_fallback_when_reserve_fails() {
    let mut value = Value::Null;
    for _ in 0..128 {
        value = Value::Array(vec![value]);
    }
    let owner = SafeJsonValue::new(value);
    SafeJsonValue::fail_next_drop_reserve_for_test();
    drop(owner);
    SafeJsonValue::assert_drop_reserve_failure_consumed_for_test();
}

#[test]
fn test_owner_deep_drop_is_safe_during_unwind() {
    let result = std::panic::catch_unwind(|| {
        let mut value = Value::Null;
        for _ in 0..32_768 {
            value = Value::Array(vec![value]);
        }
        let _owner = SafeJsonValue::new(value);
        panic!("exercise guarded owner drop during unwind");
    });
    assert!(result.is_err());
}

#[test]
fn test_owner_copy_from_unwind_drops_partial_object_iteratively() {
    let mut source = Value::Null;
    for index in 0..128 {
        let mut object = serde_json::Map::new();
        object.insert(format!("k{index}"), source);
        source = Value::Object(object);
    }
    let source = SafeJsonValue::new(source);
    SafeJsonValue::panic_after_guarded_inserts_for_test(64);
    let result = std::panic::catch_unwind(|| SafeJsonValue::copy_from(source.as_value()));
    assert!(result.is_err());
    drop(source);
}

#[test]
fn test_owner_serde_unwind_drops_partial_array_iteratively() {
    let text = format!("{}0{}", "[".repeat(128), "]".repeat(128));
    SafeJsonValue::panic_after_guarded_inserts_for_test(64);
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    deserializer.disable_recursion_limit();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        SafeJsonValue::deserialize(&mut deserializer)
    }));
    assert!(result.is_err());
    SafeJsonValue::assert_guarded_insert_panic_consumed_for_test();
}

#[test]
fn test_builder_returns_guarded_value_and_stops_after_first_error() {
    let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Integer64);
    assert!(
        builder
            .try_accept(JsonLexicalEvent::BeginObject { offset: 0 })
            .is_ok()
    );
    assert!(
        builder
            .try_accept(JsonLexicalEvent::Key {
                token: br#""a""#,
                offset: 1
            })
            .is_ok()
    );
    assert!(
        builder
            .try_accept(JsonLexicalEvent::Null { offset: 5 })
            .is_ok()
    );
    let duplicate = builder.try_accept(JsonLexicalEvent::Key {
        token: br#""a""#,
        offset: 9,
    });
    assert!(duplicate.is_err());
    assert!(
        builder
            .try_accept(JsonLexicalEvent::EndObject { offset: 12 })
            .is_err()
    );
    assert!(builder.finish_guarded().is_err());
}

#[test]
fn test_owner_serde_bridge_does_not_guess_private_map() {
    let text = r#"{"$serde_json::private::Number":"7"}"#;
    let owner: SafeJsonValue = serde_json::from_str(text).unwrap();
    assert!(owner.as_value().is_object());
    assert_eq!(
        owner
            .as_value()
            .get("$serde_json::private::Number")
            .and_then(Value::as_str),
        Some("7")
    );
}

#[test]
fn test_owner_bridge_rejects_decoded_duplicate_once() {
    let error = serde_json::from_str::<SafeJsonValue>(r#"{"a":0,"\u0061":1}"#).unwrap_err();
    assert!(error.is_data());
    assert!(error.line() >= 1);
    assert!(error.column() >= 1);
}
