# Migrate to `qubit-json` 0.12

[简体中文](migration_0_12.zh_CN.md)

This guide covers the source and behavior changes from `qubit-json` 0.11 to 0.12. The candidate requires Rust 1.94 and pairs `qubit-json` 0.12 with `qubit-budget` 0.9.1.

## f32 value and object-key representation

This checkout fixes f32 materialization and object-key consistency by using serde_json's f32 JSON representation. A `1.0_f32` object key changes from `"1"` to `"1.0"`, matching direct text encoding. Update lookups that depend on the old key; when an integer key is intended, explicitly serialize it as an integer or string. Finite f32 values can also be materialized and strictly encoded when `arbitrary_precision` is enabled. This checkout change does not claim that a release containing it has been published. No downstream source migration or version adjustment is required.

## Update dependency versions together

Use one `qubit-budget` version for every JSON limits, resource, session, and transaction type in the dependency graph:

```toml
[dependencies]
qubit-json = "0.12"
qubit-budget = { version = "0.9.1", features = ["json"] }
serde_json = "1.0"
```

`qubit-budget` 0.8 and 0.9.1 expose distinct public type identities. A `JsonDecodeLimits`, `JsonEncodeLimits`, resource, or transaction from one version cannot be passed to the other version's APIs. Remove old direct JSON-path budget pins in downstream crates, regenerate the lockfile, and compile the actual JSON exchange path. Do not add a conversion wrapper or keep both budget versions to bridge these types.

The seven direct downstream candidates that exchange these JSON/budget types are:

| Crate | Current | Candidate |
| --- | ---: | ---: |
| `qubit-datatype` | 0.14.0 | 0.15.0 |
| `qubit-redact` | 0.9.0 | 0.10.0 |
| `qubit-value` | 0.12.2 | 0.13.0 |
| `qubit-config` | 0.14.3 | 0.15.0 |
| `qubit-metadata` | 0.6.2 | 0.7.0 |
| `qubit-http` | 0.15.1 | 0.16.0 |
| `qubit-web` | 0.2.0 | 0.3.0 |

Update their JSON paths to `qubit-json = "0.12"` and `qubit-budget = "0.9.1"`, then update the corresponding downstream dependency versions and locks. These are candidate versions; this guide does not claim that they have been published. Other crates that do not exchange JSON budget types are outside this list and should not be upgraded solely for this migration.

The rs-json candidate also carries the existing Cargo change from commit `ec16e78`; after that change is integrated, its manifest uses `qubit-io` 0.19. This carries forward an already selected Cargo change and is not an additional IO migration introduced by the 0.12 plan.

## Update exhaustive serializer error matches

`JsonSerializerStateError` adds `InvalidNumberProtocol`. Update exhaustive matches over that enum. A malformed hand-written Number protocol now returns `JsonSerializationErrorKind::InvalidSerializerState` with this stable reason. A malformed RawValue protocol continues to report `InvalidRawValueProtocol`.

The Number protocol is private serde_json interoperability: it is recognized only when the exact private struct name is emitted with one string payload, supplied by `serialize_str` or one `collect_str`. Empty, multi-field, skipped, non-string, or repeated payload shapes are rejected. A serializer that catches and ignores its payload error still leaves the encoder in a failed state, so the overall operation cannot return success. Error diagnostics do not expose the payload text.

Ordinary `JsonValueEncoder` handling of an actual `serde_json::Number` now constructs a Number value instead of leaving a `$serde_json::private::Number` marker object. A normal user map with that key remains an ordinary map; the key alone does not opt into the private protocol. There is no deprecated alias or compatibility facade for changed public error shapes.

Ordinary newtype structs are transparent in `JsonValueEncoder`, including a
custom newtype named `$serde_json::private::RawValue`: its string, boolean, or
`Option` payload is serialized as that payload and is not reparsed as JSON.
Code that needs raw JSON materialization should use a genuine
`serde_json::value::RawValue`, whose exact private struct protocol remains
supported. A malformed zero-length compound serializer that emits fields or
elements now fails instead of returning success with invalid JSON; valid empty
containers remain supported.

## Keep the three number boundaries separate

- **Ordinary strict text, serialization, and native Serde integer events:** negative integers must fit `i64`, non-negative integers must fit `u64`, and fractional/exponent values must convert to finite `f64`. `i128` and `u128` events are explicitly range-checked. Text decoding admits the original `NumberBytes` measurement before checking numeric range. A Serde seed sees decoded events, not the original token; it cannot enforce `NumberBytes`. If upstream `arbitrary_precision` produces a map event, the seed cannot distinguish it from a real object and preserves it as an object.
- **Already materialized `serde_json::Value` trees:** `JsonTreeReader` counts the byte length of an existing Number's current compact display representation. Under `arbitrary_precision`, it can account for an existing `1e400` without panic and count a 39-digit integer as 39 bytes. It does not convert the value to `f64` or impose ordinary text encoding range checks. This tree accounting behavior does not make ordinary or JCS encoders accept that number.
- **JCS text:** `JcsDecoder` converts every number, including integer lexemes, to finite IEEE-754 binary64. `9007199254740993` therefore rounds to `9007199254740992`; a value such as `1e400` is rejected. Duplicate keys are rejected after unescaping. JCS Value materialization retains its 127-container guard.

## Account for Display output before appending

`qubit-budget` 0.9.1 adds `JsonValueTransaction::check_admit`. It checks quantity conversion, point limits, and cumulative nodes/payload against the transaction's current staged state without applying a successful measurement. A failed check records the first `MeasuredBudgetError`, poisons the transaction, and prevents a later `commit` from publishing staged value usage.

When `Display` writes a string or object key in chunks, the encoder checks the prospective complete string/key measurement before appending each chunk. Successful previews do not deduct budget. After formatting succeeds, it calls `try_admit` once for the complete string/key, so nodes and payload are charged once. If formatting fails after a preview failure, the budget/quantity error remains the reported error.

For private RawValue/Number raw-output payloads, the complete text is still required before its JSON structure can be validated. Output-byte limits apply while text is collected. Neither `check_admit` nor the collector can control arbitrary allocations inside user `Display::fmt`, allocator metadata, or process RSS; the guarantee is about the library's logical admission and append boundary.

## Preserved operational boundaries

- Cumulative sessions continue to retain accepted input charges. A finite streaming input limit may read one probe byte to establish that the limit was exceeded; that probe is observed but not charged.
- Value measurements remain transactional and publish only after the complete operation succeeds. Bytes already accepted by an external writer cannot be rolled back and remain charged if a later write or flush fails.
- RawValue text is validated before its bytes are emitted to an external writer. An incremental operation may leave a valid accepted prefix after a later failure.
- The 127 nested-container JCS Value materialization limit remains independent of the configurable root-inclusive depth budget.
- The same fixed 127-container limit applies when a genuine `RawValue` is projected by `JsonValueEncoder`; it is a materialization guard, not a general ordinary encode/decode depth limit. `InvalidRawValue` means the strict RawValue materialization contract was rejected and is not reserved for syntax errors.
- Default diagnostics remain redacted and do not retain input, private payload, or arbitrary `Serialize::custom` text.

Private source file organization is an implementation detail. The migration does not promise old private module paths, deprecated aliases, or a bridge between budget type identities.

## Related documentation

- [User guide](user_guide.md) · [用户手册](user_guide.zh_CN.md)
- [Number contract](number_contract.md) · [数字契约](number_contract.zh_CN.md)
- [Design](json_design.md) · [设计](json_design.zh_CN.md)
