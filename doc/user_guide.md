# Qubit JSON user guide

This guide targets Rust 1.94+ applications using `qubit-json` 0.12. It is for
services, configuration readers, and data pipelines that must admit JSON under
caller-owned resource limits while keeping Serde's data model. It does not
replace `serde_json` or impose application schemas.

[中文版](user_guide.zh_CN.md) · [README](../README.md) · [Released API documentation](https://docs.rs/qubit-json)

## Purpose and audience

Use this crate at a trust boundary where input size, nesting, collection size,
number-token size, decoded values, or output bytes need explicit limits. Keep
schema validation, authorization, identifier policy, and decimal precision in
the application layer after JSON admission.

Start with the [HTTP boundary scenario](#scenario-bounded-json-at-an-http-boundary)
for the basic decode/encode path. Then use the [type and ownership reference](#core-types-and-ownership)
to choose an API, [protected values](#protected-values-validated-decoding-and-formatting)
for owned trees and token formatting, or [tree processing](#read-and-mutate-existing-trees)
for visitors. The [errors](#errors-and-diagnostics) and [troubleshooting](#troubleshooting)
sections cover failure handling.

## Conceptual model

`qubit-budget` owns resource identities, limits, transactions, and sessions.
`qubit-json` exposes four top-level modules:

- `decode` admits strict JSON text or performs explicitly configured text
  normalization before strict admission.
- `encode` serializes values to budgeted JSON output.
- `format` validates and reformats JSON tokens without constructing a value
  tree or changing scalar token spellings.
- `value` builds or validates materialized `serde_json::Value` trees from Serde
  events and supplies the protected `SafeJsonValue` owner. Its `value::traverse`
  submodule reads or mutates an existing tree without Rust recursion.

Strict decoders and encoders are stateful objects. Use `with_limits(limits)`
to create a fresh owned session, or pass an existing
`JsonDecodeSession`/`JsonEncodeSession` to `new(session)`. Both constructors
produce reusable codecs: usage accumulates across calls on the same session.
Value-resource charges
commit only after the complete operation succeeds. Serialization into a
temporary buffer does not publish output charges on failure. Once an external
destination accepts bytes, those bytes remain written and charged even if a
later write or flush fails. Input charges remain visible in the session after
a failed decode attempt.

## Scenario: bounded JSON at an HTTP boundary

Suppose an endpoint receives a JSON request body containing an identifier and a
small payload. Success means accepting the complete unsigned 64-bit identifier
range, rejecting an oversized body before a decoded value is committed, and
then handing only admitted data to application validation.

### Installation and minimal configuration

The dependency versions below match this checkout. Use a local `path`
while developing against unpublished changes:

```toml
[dependencies]
qubit-json = "0.12"
qubit-budget = { version = "0.10", features = ["json"] }
serde_json = "1.0.135"
```

Add `serde = { version = "1.0", features = ["derive"] }` when the application
decodes into derived types.

### Core workflow

The Rust examples in this guide and both READMEs are included by the root
crate's [documentation example module](../src/documentation_examples.rs). Run
them from the repository root with `cargo test --locked --doc --features jcs`.
The serde_json compatibility matrix also tests this root crate, not a downstream
fixture: it runs eight combinations of `jcs`, `serde_json/arbitrary_precision`,
and `serde_json/preserve_order` against baseline `1.0.135` and the latest
resolvable version. The baseline follows the API boundary: `Map::into_values()`
was added in serde_json 1.0.135. If an existing `Cargo.lock` pins an older
release, run `cargo update -p serde_json` before the `--locked` checks. The
minimum does not require `arbitrary_precision` or `preserve_order`.

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;

fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .max_depth(32)
        .max_nodes(256)
        .max_sequence_items(64)
        .max_map_entries(64)
        .max_key_bytes(128)
        .max_string_bytes(2048)
        .max_number_bytes(20)
        .max_payload_bytes(4096)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);

    let request_body = br#"{"id":18446744073709551615,"ok":true}"#;
    let value: serde_json::Value = decoder.decode_utf8(request_body)?;
    assert_eq!(value["id"], serde_json::json!(u64::MAX));

    let small_limits = limits.into_builder().max_input_bytes(8).build();
    let mut small_decoder = JsonDecoder::with_limits(small_limits);
    let error = small_decoder
        .decode_utf8::<serde_json::Value>(br#"{"ok":true}"#)
        .expect_err("the request body must exceed eight bytes");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.raw_input_bytes(), 11);
    Ok(())
}
```

The observable success result is an admitted `serde_json::Value`. The failure
branch returns `JsonDecodeErrorKind::Budget` and reports 11 observed input bytes.
That accessor describes bytes observed by this call, not bytes committed to a
cumulative budget: the rejected input charge is rolled back with the failed
value attempt, while a previously accepted input charge remains committed. This lets an HTTP adapter map
stable error categories to responses without matching private parser details.

For output, create `JsonEncoder::with_limits(JsonEncodeLimits::...)` and call
`to_vec`, `write_buffered`, or `write_incremental`:

```rust
use qubit_budget::json::JsonEncodeLimits;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use serde_json::json;

let value = json!({"status": "ok"});
let mut buffered = JsonEncoder::with_limits(
    JsonEncodeLimits::builder().max_output_bytes(64).build(),
);
assert_eq!(buffered.to_vec(&value)?, br#"{"status":"ok"}"#);

let mut incremental = JsonEncoder::with_limits(
    JsonEncodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_output_bytes(4)
        .build(),
);
let mut output = Vec::new();
let error = incremental
    .write_incremental(&mut output, &[1, 2, 3])
    .expect_err("four output bytes cannot hold the complete array");
assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
assert_eq!(output, b"[1,2");
# Ok::<(), qubit_json::encode::JsonEncodeError<qubit_budget::json::JsonResource>>(())
```

`to_vec` serializes into a temporary buffer, so a failed serialization does not
publish output charges. Buffered mode finishes serialization and budget checks
before touching the writer. If a later write fails, bytes already accepted by
the external writer remain written and charged, while value-resource charges
are rolled back. See the existing
[`test_buffered_writer_failure_keeps_prefix_and_rolls_back_value`](../tests/encode/json_encode_session_tests.rs)
regression: the writer accepts two bytes before failing, leaving `output = 2`
and `nodes = 0`. Incremental mode likewise retains and charges accepted prefixes
if a later serialization, budget, or writer operation fails.

`write_buffered` returns success when the writer accepts all bytes and the
encoding attempt commits its accounting. It does not flush the writer. If you
pass a buffered writer, flush it after the call succeeds; that later flush is
outside the encoder transaction, so a flush failure does not roll back the
committed charges. For example:

```rust
use std::io::{BufWriter, Write};
use qubit_budget::json::{JsonEncodeLimits, JsonResource};
use qubit_json::encode::JsonEncoder;
use serde_json::json;

let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
    .max_output_bytes(64)
    .build();
let mut encoder = JsonEncoder::with_limits(limits);
let mut writer = BufWriter::new(Vec::new());
encoder.write_buffered(&mut writer, &json!({"ok": true}))?;
writer.flush()?;
let bytes = writer.into_inner().expect("flush already succeeded");
assert_eq!(bytes, br#"{"ok":true}"#);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`write_incremental` flushes after serialization and before committing
value-resource accounting. If that flush fails, the method returns a `Write`
error: bytes already accepted remain charged to the output budget, and value
resources are not committed. Both methods borrow the writer through
`&mut O`; the caller retains ownership after the call. Do not rely on `Drop` for an
implicit flush. A successful flush reports only the writer's result; it does
not confirm that bytes reached disk or a remote peer.

The next step is to deserialize the admitted document into an application type
and apply schema, authorization, and identifier rules. Continue below when the
boundary also needs normalization, repeated decoding, unique object keys, or
tree processing.

### Core types and ownership

The scenario above uses a decoder for input and an encoder for output. Choose
additional types according to what the application needs to retain or inspect.
Here, `Value` means `serde_json::Value`; arrows describe successful results,
and fallible calls return `Result`.

| Task | Public API and data shape | Ownership and accounting |
| --- | --- | --- |
| Decode into an application type | `JsonDecoder::decode_str::<T>` / `decode_utf8::<T>`: text → `T` | Slice results may borrow input; `decode_input::<T>` requires `DeserializeOwned` and buffers the document. The decoder owns its cumulative session. |
| Retain a protected JSON tree | `JsonDecoder::decode_value_*`: text → `SafeJsonValue` | Owns the decoded tree and rejects duplicate keys. Own-tree operations use explicit stacks; admission uses the decoder's configured limits. |
| Decode after permitted text cleanup | `NormalizingJsonDecoder`: text → owned `T`; `prepare_*` → `NormalizedJsonDocument` | Policy controls transformations. The document retains `Cow<'input, str>` and input byte lengths; document-based results can borrow its text. |
| Emit strict JSON | `JsonEncoder::to_vec(&T)` → `Vec<u8>`; `write_*(&mut output, &T)` → `()` | Accepts `T: Serialize` and retains a cumulative encode session. External output can retain accepted bytes after failure. |
| Project Serde data into a tree | `JsonValueEncoder::encode(&T)` → `Value`; `encode_in_transaction` → `SafeJsonValue` | The encoder is stateless. `encode` has no configured resource limits; the transaction-aware method stages caller-owned value accounting. |
| Read or modify an existing tree | `JsonTreeReader::process(&Value, &mut visitor)` / `JsonTreeMutator::process(&mut Value, &mut visitor)` | Borrows value transactions. Mutators use separate input and output transactions; dropping them does not undo tree mutations. |
| Sign or hash a canonical representation | `JcsDecoder::decode_value_*`: text → `SafeJsonValue` (legacy `decode_*` → `Value`); `JcsEncoder`: `&Value` → canonical bytes | Requires `jcs`. Numbers use binary64 semantics; encoder working capacity is configured separately from value/output budgets. |
| Pretty-print without changing tokens | `format_json_tokens` / `write_json_tokens` + `JsonPrettyOptions` | Operates on JSON bytes, preserving scalar spellings and duplicate keys. Its output limit is separate from codec sessions. |

The tree model is `Null | Bool | Number | String | Array(Vec<Value>) | Object(Map<String, Value>)`.
`SafeJsonValue` wraps that model rather than introducing another set of node
types. `as_value()` borrows it for tree readers and interoperability;
`into_value()` transfers the bare tree to the caller.

Codec generics have the shape `<'budget, R = JsonResource, Q = usize>`:
`'budget` is the lifetime of any borrowed budget, `R` identifies resources,
and `Q` is the quantity type. The defaults are sufficient for the examples in
this guide. Use `session()` to inspect accounting or `into_session()` to
recover a codec's session when transferring it to another codec.

## Canonical bytes for signatures and hashes

When an application signs or hashes JSON, the sender and verifier need the
same byte representation even if object members arrived in a different order.
Enable the optional `jcs` feature and give `JcsEncoder` a constructed
`serde_json::Value`:

```toml
[dependencies]
qubit-json = { version = "0.12", features = ["jcs"] }
serde_json = "1.0.135"
```

The application can then canonicalize a value before passing its bytes to its
existing signing or hashing code:

```rust
# #[cfg(feature = "jcs")]
use qubit_budget::json::{JsonEncodeLimits, JsonResource};
# #[cfg(feature = "jcs")]
use qubit_json::encode::JcsEncoder;

# #[cfg(feature = "jcs")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(4096)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let value = serde_json::json!({"b": 2, "a": 1.0});
    let canonical = encoder.to_vec(&value)?;
    assert_eq!(canonical, br#"{"a":1,"b":2}"#);
    Ok(())
}
# #[cfg(not(feature = "jcs"))]
# fn main() {}
```

The feature is disabled by default. The encoder accepts `Value`, not arbitrary
`Serialize` implementations or raw JSON text. Use `JcsDecoder` to reject
duplicate keys while reading raw JSON, before constructing `Value`; once
duplicate members have been collapsed into a map, the encoder cannot detect
them. The existing `JsonDecoder` is a strict JSON decoder, but its integer
text limits do not cover every number lexeme accepted by JCS. Use `JcsDecoder` as the complete
raw-text entry point when JCS number and duplicate-key rules are required.

JCS numbers follow the IEEE 754 binary64 model. A value containing
`9007199254740993` can therefore produce the canonical number
`9007199254740992`. Represent an integer as a JSON string when its exact digits
must survive canonicalization. With `arbitrary_precision`, a `Number` that is
stored but cannot be represented as a finite binary64 value is still rejected
by the JCS encoder. The inner serialization error is `NonFiniteFloat` in the
`Number` category; the outer JCS error remains `Serialize`. Diagnostics do not
retain the original number token.

`to_vec` and `to_string` return a complete result. Serialization into their
temporary buffer does not publish output charges on failure. `write_buffered`
creates and checks the canonical document before writing it; `write_incremental`
sends produced bytes directly to the destination. Either writer can leave a
prefix if a write fails, and accepted bytes remain charged to the output
budget; value-resource charges commit only if the complete operation succeeds.
Incremental output is flushed before success is reported, so accepted bytes
remain charged if that flush fails. A write or flush failure is `Write`; an
output limit failure is `Budget`; exceeding the separate
`max_working_bytes` auxiliary-capacity limit is `WorkingMemory`. Working memory
covers the renderer's traversal stack and active object member tables, not the
input `Value`, allocator metadata, or output buffer capacity. `unlimited()`
selects unlimited value, output, and working-memory budgets, so reserve it for
trusted or separately admitted data.

`JcsEncoder::write_buffered` does not flush its destination. Pass `&mut writer`
when using a buffered writer, and explicitly flush after a successful call.
That later flush is outside the committed encoding transaction, so failure
does not undo its charges. Do not rely on `Drop` to report flush failures. By
contrast, `write_incremental` flushes before committing value accounting. A
successful flush does not imply durable storage or remote delivery. See the
[buffered writer example above](#scenario-bounded-json-at-an-http-boundary).

Value admission runs before rendering when value limits are configured. Its
traversal stack is not covered by `max_working_bytes`; set `max_depth` to bound
that pass and `max_nodes` to bound the admitted tree. For example, a service
that accepts a materialized value can combine finite value, output, and
renderer limits as follows:

```rust
# #[cfg(feature = "jcs")]
# fn main() -> Result<(), qubit_json::encode::JcsEncodeError<qubit_budget::json::JsonResource>> {

use qubit_budget::json::{JsonEncodeLimits, JsonResource};
use qubit_json::encode::JcsEncoder;

let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
    .max_depth(32)
    .max_nodes(1024)
    .max_output_bytes(64 * 1024)
    .build();
let mut encoder = JcsEncoder::with_limits(limits, 8 * 1024);
let canonical = encoder.to_vec(&serde_json::json!({"status": "ready"}))?;
assert_eq!(canonical, br#"{"status":"ready"}"#);
# Ok::<(), qubit_json::encode::JcsEncodeError<qubit_budget::json::JsonResource>>(())
# }
# #[cfg(not(feature = "jcs"))]
# fn main() {}
```

These limits bound configured logical resources and renderer collection
capacities. They do not promise an exact heap limit or include the existing
input `Value`, admission stack, allocator metadata, or output buffer.

### Decode raw text with JCS semantics

For raw JSON text, use the feature-gated `JcsDecoder` instead of first building
a `Value` with ordinary `JsonDecoder`. Prefer `decode_value_str`,
`decode_value_utf8`, and `decode_value_input` when the result may be deeply
nested; they return `SafeJsonValue` directly. The older `decode_str`,
`decode_utf8`, and `decode_input` methods remain for compatibility and return
bare `serde_json::Value`. Pass the protected tree's borrowed view to
`JcsEncoder::to_vec`. This example sets finite input, structure, and value
limits, then encodes the protected value:

```rust
# #[cfg(feature = "jcs")]
use qubit_budget::json::{JsonDecodeLimits, JsonEncodeLimits, JsonResource};
# #[cfg(feature = "jcs")]
use qubit_json::decode::JcsDecoder;
# #[cfg(feature = "jcs")]
use qubit_json::encode::JcsEncoder;

# #[cfg(feature = "jcs")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let decode_limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .max_depth(16)
        .max_nodes(32)
        .max_map_entries(8)
        .max_number_bytes(32)
        .max_string_bytes(256)
        .max_key_bytes(64)
        .build();
    let mut decoder = JcsDecoder::with_limits(decode_limits);
    let value = decoder
        .decode_value_str(r#"{"b":100000000000000000000,"a":1.0}"#)?;

    let encode_limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(4096)
        .build();
    let mut encoder = JcsEncoder::with_limits(encode_limits, 16 * 1024);
    let canonical = encoder.to_vec(value.as_value())?;
    assert_eq!(canonical, br#"{"a":1,"b":100000000000000000000}"#);
    Ok(())
}
# #[cfg(not(feature = "jcs"))]
# fn main() {}
```

Every number, whether written as an integer, fraction, or exponent, is rounded
to binary64 before it is stored in `Value`. For example,
`9007199254740993` becomes `9007199254740992`; `1e400` is rejected because it
is non-finite, while sufficiently small values may underflow to zero. Encode
amounts and identifiers as JSON strings when their exact digits must survive,
then parse them according to the application's domain rules.

Duplicate detection compares decoded JSON key strings. This executable example
shows that a Unicode escape can spell the same key, while a doubled backslash
makes the backslash part of a different key:

```rust
# #[cfg(feature = "jcs")]
# fn main() {

use qubit_json::decode::{JcsDecodeError, JcsDecoder};

let error = JcsDecoder::unlimited()
    .decode_str(r#"{"x":1,"\u0078":2}"#)
    .unwrap_err();
assert!(matches!(error, JcsDecodeError::DuplicateKey { .. }));
let value = JcsDecoder::unlimited()
    .decode_str(r#"{"x":1,"\\u0078":2}"#)
    .unwrap();
assert_eq!(value.as_object().unwrap().len(), 2);
# }
# #[cfg(not(feature = "jcs"))]
# fn main() {}
```



The decoder uses the configured, root-inclusive `max_depth` for admission;
there is no separate 127-container construction guard. The `decode_value_*`
methods return `SafeJsonValue` directly and are the recommended entry points
for deep documents. Legacy `decode_*` methods return bare
`serde_json::Value` for compatibility; protect such a result immediately if
you keep using that API:

```rust
# #[cfg(feature = "jcs")]
# fn main() {

use qubit_json::decode::JcsDecoder;
use qubit_json::value::SafeJsonValue;

for depth in [128, 129] {
    let nested = (0..depth).fold(String::from("null"), |value, _| format!("[{value}]"));
    let value = JcsDecoder::unlimited().decode_str(&nested).unwrap();
    let protected = SafeJsonValue::new(value);
    assert!(protected.as_value().is_array());
}
# }
# #[cfg(not(feature = "jcs"))]
# fn main() {}
```

Handle JCS errors by variant: `Decode(source)` carries ordinary input,
syntax, and budget failures; `DuplicateKey { line, column }` identifies the
second key's opening quote. `MaterializationDepthLimit { limit, line, column }`
remains as a historical compatibility variant; current depth rejection comes
from the configured budget and is wrapped in `Decode`. Public error families
also include `JcsEncodeError`, `JsonSerializationError`, and the ordinary
`JsonDecodeError` / `JsonEncodeError` domain errors.

`JsonValueEncoder` uses the shared builder when materializing a genuine
`serde_json::value::RawValue`; its transaction-aware entry point returns a
protected owner. `JsonSerializationErrorKind::InvalidRawValue` identifies
rejection by the strict RawValue contract, including duplicate keys, and does
not mean only that the bytes are syntactically invalid.

Keys are compared after JSON unescaping, so `"x"` and `"\u0078"` are
duplicates, including in nested objects. Unicode is not normalized to NFC or
NFD, so different code-point sequences remain distinct keys. Valid JSON
whitespace and member ordering need not be canonical. Comments, a BOM, trailing
commas, invalid UTF-8, unpaired surrogates, and a second trailing document are
rejected. This entry point does not verify that input bytes are already
canonical.

`decode_input` reads through EOF and buffers the full input to return an owned
`Value`. With a finite `max_input_bytes`, it may consume one additional probe
byte to distinguish EOF at the exact limit from overflow. That byte is not
charged or buffered and cannot be returned to the input source. Accepted input
bytes remain charged if later decoding fails; value resources commit only after
the full operation succeeds.

Resource budgets limit measured JSON resources; they are not exact malloc
ceilings. Input buffers, `Vec`/`Map` capacities, and allocator metadata are not
measured precisely. Lexical depth is root-inclusive and follows the configured
`max_depth`. JCS Value construction uses the shared builder and has no separate
fixed container cap. The legacy `MaterializationDepthLimit` variant remains
for source compatibility; configured depth rejection is reported as
`JcsDecodeError::Decode` with a budget error. The `decode_value_str`,
`decode_value_utf8`, and `decode_value_input` methods return `SafeJsonValue`
directly and are the recommended entry points for deep trees. Only the
compatibility methods `decode_str`, `decode_utf8`, and `decode_input` return a
bare `Value`; if using one of them, wrap a deep result immediately with
`SafeJsonValue::new` before retaining or dropping it.

## Protected values, validated decoding, and formatting

### Retain and borrow a protected tree

For the HTTP scenario, use `decode_value_utf8` instead of a generic `Value`
target when the application needs an owned tree with duplicate-key rejection
and protected tree operations. The following standalone example verifies both
successful ownership and duplicate-key rejection:

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_input_bytes(256)
    .max_depth(8)
    .max_nodes(16)
    .max_payload_bytes(128)
    .build();
let mut decoder = JsonDecoder::with_limits(limits);
let owner = decoder.decode_value_utf8(br#"{"id":42,"ok":true}"#)?;
assert_eq!(owner.as_value()["id"], 42);
assert_eq!(owner.clone(), owner);

let error = decoder.decode_value_str(r#"{"id":42,"id":43}"#).unwrap_err();
assert_eq!(
    error.materialization_error_reason(),
    Some(qubit_json::decode::JsonMaterializationErrorReason::DuplicateKey),
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Pass `owner.as_value()` to readers while keeping the owner alive. For a tree
already constructed elsewhere, `SafeJsonValue::new(value)` takes ownership
without budget admission; `copy_from(&value)` copies it with an explicit stack.
`into_value()` gives up the owner's cleanup protection, so retain the owner
when deep-tree destruction must stay iterative. Calling recursive operations
on a borrowed bare `Value` is outside that protection.

When a caller only needs a short-lived read-only view, `JsonValueRef` borrows
the existing tree and implements `Serialize` without cloning it:

```rust
use qubit_json::value::{JsonValueRef, SafeJsonValue};

fn serialize_view(owner: &SafeJsonValue) -> Result<String, serde_json::Error> {
    let view = JsonValueRef::new(owner.as_value());
    serde_json::to_string(&view)
}

let owner = SafeJsonValue::new(serde_json::json!({"a": true}));
assert_eq!(serialize_view(&owner)?, r#"{"a":true}"#);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The view lifetime is bounded by the borrowed value. Constructing it is O(1)
and does not copy the tree; comparisons, `Debug`, and `Serialize` still walk
the tree and may allocate according to their implementation or serializer.
Keep the owner alive for the full view use. Its serializer preserves the
underlying `Value` data model, but the output spelling and formatting remain
the serializer's choice.

### Inspect original number tokens

Use `decode_unique_validated_*` when policy must inspect original number tokens
with their source paths. The following path applies per-document strategy
limits and records token metadata during one scan:

```rust
use qubit_json::decode::{
    JsonDecoder, JsonNumberPolicy, JsonPathSegment, JsonValidatedDecodeOptions,
};

let options = JsonValidatedDecodeOptions::builder()
    .max_value_and_key_nodes(32)
    .max_encoded_string_bytes(128)
    .build();
let mut decoder = JsonDecoder::unlimited();
let mut numbers = Vec::new();
let owner = decoder.decode_unique_validated_str(
    r#"{"id":42,"ratio":1.25}"#,
    &options,
    |path, token| {
        numbers.push((path.to_vec(), token.raw().to_owned(), token.offset()));
        let policy = if matches!(path.last(), Some(JsonPathSegment::Key(key)) if key == "id") {
            JsonNumberPolicy::ExactSafeInteger
        } else {
            JsonNumberPolicy::Preserve
        };
        Ok::<_, std::convert::Infallible>(policy)
    },
)?;
assert_eq!(numbers.len(), 2);
assert!(owner.as_value().is_object());
# Ok::<(), Box<dyn std::error::Error>>(())
```

On success, the validated call returns a `SafeJsonValue`. Keep that owner
alive while borrowing its tree through `as_value()`; calling `into_value()`
deliberately gives up the owner's protected lifecycle. The path callback sees
decoded object keys and array indexes, while `token.raw()` retains the source
spelling of each number. Select `ExactSafeInteger` only for paths whose values
must fit the exact safe-integer range; return `Preserve` for other numeric
fields.

`JsonValidatedDecodeOptions` limits the number of value-and-key nodes and the
encoded bytes in each string token for that document. The decoder's
`JsonDecodeSession` still accumulates its configured resource usage across
calls. Policy callback effects are application effects: a later syntax error
does not undo them.

Validated decoding defaults to `JsonValidatedAdmissionPolicy::Immediate`,
which preserves the existing first-error order. Call
`JsonValidatedDecodeOptions::builder().admission_policy(
JsonValidatedAdmissionPolicy::TokenFirst)` only when the application wants a
complete token admitted before checking its following delimiter. For example,
with TokenFirst, `{"a":truex}` admits the complete `true` token and then
reports the `x` at the object's separator, exposed as
`JsonDecodeContext::ObjectSeparator`:

```rust
use qubit_json::decode::{
    JsonDecodeContext, JsonDecoder, JsonNumberPolicy,
    JsonValidatedAdmissionPolicy, JsonValidatedDecodeError,
    JsonValidatedDecodeOptions,
};

let options = JsonValidatedDecodeOptions::builder()
    .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
    .build();
let mut decoder = JsonDecoder::unlimited();
let error = decoder
    .decode_unique_validated_str("{\"a\":truex}", &options, |_path, _token| {
        Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve)
    })
    .expect_err("the object still requires a separator after true");
match error {
    JsonValidatedDecodeError::Decode { error, context } => {
        assert_eq!(context, Some(JsonDecodeContext::ObjectSeparator));
        assert_eq!(error.syntax_error().unwrap().context(), context);
    }
    other => panic!("unexpected error: {other:?}"),
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

`TokenFirst` remains strict JSON. It validates a number's complete candidate
from `[0-9eE+.-]` before a delimiter error: `1-2` is an invalid number, while
`1x` admits `1` and then fails at root trailing input or the enclosing
container separator. For strings, it first finds the escape-aware closing
quote and checks the encoded-byte limit, then validates and decodes the
contents.

This option is specific to the validated decode APIs. Typed `JsonDecoder`
decoding, pretty formatting, and JCS decoding keep their existing admission
rules; choosing a shared scanner internally does not opt them into TokenFirst.

The encoded-string limit counts source bytes inside the quotes, including
escape spelling. With a maximum of one byte, an unclosed `"ab` reports a
syntax `UnexpectedEnd` at EOF because the scanner checks for EOF before the
next byte's limit decision. The complete `"ab"` reports the
`JsonValidatedDecodeError::Limit` variant for `EncodedStringBytes` instead.
The unclosed over-limit `"abc` reports that same limit first: after `"ab` has
exceeded the maximum, the scanner sees the next available byte before it can
reach EOF. This distinction makes the first error depend on whether the
over-limit prefix has another byte to inspect.

Input accounting and value transactions have different commit boundaries.
Input bytes already read remain charged when validated decoding later fails;
value-resource charges commit only after the entire document succeeds. A
number-policy callback is application code, so effects it performs are not
rolled back when syntax or a later limit fails.

Generic Serde deserialization has an additional boundary: with
`serde_json/arbitrary_precision`, an upstream deserializer can present the
number representation through Serde's private map protocol, which a generic
`SafeJsonValue` bridge cannot reliably distinguish from an ordinary map. If
JSON text and number-token semantics matter, prefer `JsonDecoder` and its
validated entry points, which scan the source text directly.

### Project values and format their output

Projection can add a separate value-budget transaction. A successful batch is
committed only after every projection succeeds:

```rust
use qubit_budget::json::{JsonResource, JsonValueBudget, JsonValueLimits};
use qubit_json::encode::JsonEncoder;
use qubit_json::format::{JsonPrettyOptions, format_json_tokens};
use qubit_json::value::JsonValueEncoder;
use serde_json::json;

let limits = JsonValueLimits::<JsonResource, usize>::builder()
    .max_nodes(16)
    .build();
let mut budget = JsonValueBudget::new(limits);
let mut transaction = budget.transaction();
let encoder = JsonValueEncoder::new();
let first = encoder.encode_in_transaction(&json!({"ok": true}), &mut transaction)?;
let second = encoder.encode_in_transaction(&json!([1, 2]), &mut transaction)?;
transaction.commit()?;
assert!(first.as_value().is_object());
assert!(second.as_value().is_array());
let encoded = JsonEncoder::unlimited().to_vec(&first)?;
let pretty_options = JsonPrettyOptions::builder().build()?;
let pretty = format_json_tokens(&encoded, &pretty_options, None)?;
assert!(std::str::from_utf8(&pretty)?.contains("\"ok\": true"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

If any projection returns an error or unwinds, drop the entire transaction;
do not commit staged usage from a failed batch. The returned owners can then be
passed to `JsonEncoder`, followed by `format_json_tokens` for whitespace-only
layout. Pretty formatting validates the full JSON syntax before output, keeps
the original number and string token spellings, and permits duplicate object
keys because it does not construct a `Value`. I/O failures can still leave an
accepted prefix in the destination.

For generated RawValue or Number payloads, the bounded entry point limits each
private payload's UTF-8 source bytes before the library copies or appends past
the maximum:

```rust
use qubit_budget::json::{JsonResource, JsonValueBudget, JsonValueLimits};
use qubit_json::value::JsonValueEncoder;
use serde_json::value::RawValue;

let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
let mut transaction = budget.transaction();
let raw = RawValue::from_string("{\"ok\":true}".to_owned())?;
let owner = JsonValueEncoder::new()
    .encode_in_transaction_with_private_text_limit(&raw, &mut transaction, 64)?;
transaction.commit()?;
assert_eq!(owner.as_value()["ok"], true);
# Ok::<(), Box<dyn std::error::Error>>(())
```

The inclusive limit applies independently to each private payload; it is not
cumulative, does not limit ordinary strings, and does not measure allocation
capacity or total process memory. The older `encode_in_transaction` entry
point has no private-text limit. This limit is separate from `JsonEncoder`'s
output-byte limit. On `JsonValueEncodeError::PrivateTextLimitExceeded { maximum }`,
drop the whole transaction instead of committing staged measurements.

`JsonPrettyOptions::builder()` defaults to two spaces per level, no displayed
depth cap, and no trailing newline. `max_indent_levels` changes indentation
only; it does not restrict accepted nesting. The third argument to
`format_json_tokens` is an optional formatted-output byte limit, not an input,
depth, or heap-memory limit. `write_json_tokens` validates and plans the full
document before writing, leaves output untouched on syntax or output-limit
failure, and does not flush the destination. A later I/O failure can leave an
accepted prefix; reject the operation rather than treating that prefix as a
complete document.

```rust
use qubit_json::format::{JsonPrettyOptions, format_json_tokens};

let options = JsonPrettyOptions::builder().build()?;
let formatted = format_json_tokens(
    br#"{"x":1e400,"x":1.00,"s":"\u0061"}"#,
    &options,
    None,
)?;
assert!(std::str::from_utf8(&formatted)?.contains("1e400"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

`JsonDecoder::decode_value_*` returns `SafeJsonValue` with protected owned-tree
operations. The older typed `decode_*<T>` methods retain their Serde target
semantics and do not promise arbitrary-depth construction or destruction.
Tree protection and Serde input semantics are separate: deserializing a
`SafeJsonValue` directly cannot distinguish a real object from an upstream
deserializer's private number map. With `arbitrary_precision`, such bridge
events may remain ordinary objects, so their numeric shape can differ across
feature configurations. For JSON text whose numeric meaning must be preserved,
decode through `JsonDecoder`:

```rust
use qubit_json::decode::JsonDecoder;

let value = JsonDecoder::unlimited().decode_value_str("1.25")?;
assert_eq!(value.as_value().as_f64(), Some(1.25));
# Ok::<(), Box<dyn std::error::Error>>(())
```

Choose a finite `max_depth` for untrusted data; `None` means no configured
depth ceiling, not unlimited memory use.

## Advanced usage

Use `NormalizingJsonDecoder` only when the boundary explicitly permits the
configured transformations (BOM, surrounding whitespace, one JSON Markdown
fence, or control-character escaping). Its policy is independent from
`JsonDecodeLimits`; pass them to `NormalizingJsonDecoder::with_limits(policy,
limits)`, or pass a `JsonDecodeSession` to `NormalizingJsonDecoder::new(policy,
session)`.

Slice-based strict and normalizing decoder methods consume the complete
supplied `&str` or `&[u8]`. Their input-byte limit admits that slice; it does
not retroactively limit memory already allocated by an HTTP body aggregator or
other transport. Strict `JsonDecoder` slice methods can return values borrowing
from the input. One-shot `NormalizingJsonDecoder` methods require
`DeserializeOwned` because normalized text may be temporary. To borrow after
normalization, prepare a document and decode from it; borrowed results cannot
outlive the `NormalizedJsonDocument` they reference.
`JsonDecoder::decode_input` offers a byte-oriented source for owned results:

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecoder;
fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut body = std::io::Cursor::new(br#"{"id":42}  "#);
    let request: serde_json::Value = decoder.decode_input(&mut body)?;
    assert_eq!(request["id"], 42);
    Ok(())
}
```

`decode_input` accepts `Input<Item = u8>` sources; standard `Read` values such
as `Cursor` use the `qubit-io` integration. It reads through EOF and applies
strict checks to the complete document, including trailing whitespace and any
second JSON value. With a configured `max_input_bytes`, each read is limited
to the remaining budget plus at most one probe byte. The probe distinguishes
EOF at the exact limit from additional input. Bytes within the limit are
charged and buffered; if the probe reads data, decoding fails with a budget
error. `Input` cannot return that probe byte to its source, and the probe is
neither charged nor included in the buffered JSON or reported input length.
The return type must implement `DeserializeOwned` because the decoder buffers
the complete document internally. With no input-byte limit, that temporary
buffer has no configured maximum. This method parses ordinary strict JSON; it
does not add the number-text rules or duplicate-key rejection needed to
canonicalize arbitrary raw JSON as JCS.

### Prepared normalized documents

The one-shot normalizing methods return owned targets because temporary
normalized text cannot outlive the call. For borrowing, seed-driven decoding,
or repeated materialization, prepare a document explicitly:

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;

fn main() -> Result<(), JsonDecodeError> {
    let limits = JsonDecodeLimits::builder()
        .max_input_bytes(1024)
        .max_normalized_input_bytes(1024)
        .max_depth(16)
        .max_nodes(64)
        .max_sequence_items(32)
        .max_map_entries(32)
        .max_key_bytes(128)
        .max_string_bytes(512)
        .max_number_bytes(32)
        .max_payload_bytes(1024)
        .build();
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::lenient(),
        limits,
    );
    let document = decoder.prepare_str("  \"borrowed\"  ")?;
    let first: &str = decoder.decode_precharged_document(&document)?;
    let second: &str = decoder.decode_precharged_document(&document)?;
    assert_eq!((first, second), ("borrowed", "borrowed"));
    Ok(())
}
```

`prepare_str`/`prepare_utf8` immediately commit raw and normalized input
charges. The `decode_precharged_*` and `validate_precharged_document` names
make that input-accounting precondition explicit. Each successful
`decode_precharged_document`, `decode_precharged_document_seed`,
`decode_precharged_object_document`, `decode_precharged_array_document`, or
`validate_precharged_document` commits a separate value charge; a failed
materialization rolls back only that attempt's value charges. The document is
detached and may be decoded by another compatible decoder. Its normalized text
borrows the original input when normalization needs no rewrite, and owns the
text when rewriting requires allocation. Borrowed decode results are tied to
the prepared document and cannot outlive it. Borrowing follows Serde's
representation rules: unescaped JSON strings can
borrow from the document, while escaped strings require owned targets.

### Integrate a caller-owned Serde deserializer

Use `AccountingJsonValueSeed` when another Serde deserializer owns the input. It charges
decoded events and enforces `NumberBytes` using the compact display length of each
constructed `Number`, but cannot measure the original number token or enforce
text-level integer range. For example, with the default number representation, the
three-byte token `1e2` becomes `100.0`: a limit of 3 rejects it and a limit of 5
accepts it. See the [number contract](number_contract.md). Use `JsonDecoder` when
you need limits on the original text. `DuplicateKeyRejectingJsonValueSeed` rejects
duplicate keys but does not apply a resource budget.
Use `decode_object_str`/`decode_object_utf8` or the corresponding array methods
when the boundary also requires a specific top-level container.

### Duplicate object keys

Strict admission does not add a global unique-key rule. Duplicate-key behavior
belongs to the requested Serde target: `serde_json::Value` and
`serde_json::Map` use last-key-wins semantics, while many derived structs
reject duplicate known fields. Choose the target that expresses the document
contract instead of assuming that `strict` includes key uniqueness.

Use `decode_unique_str` with `DuplicateKeyRejectingJsonValue` when every
object must have unique keys and numbers must survive `arbitrary_precision`
feature unification:

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::value::DuplicateKeyRejectingJsonValue;

fn main() -> Result<(), JsonDecodeError<JsonResource>> {
    let input = r#"{"role":"user","role":"admin"}"#;
    let limits = JsonDecodeLimits::builder()
        .max_input_bytes(1024)
        .max_depth(16)
        .max_nodes(64)
        .max_sequence_items(16)
        .max_map_entries(16)
        .max_key_bytes(64)
        .max_string_bytes(256)
        .max_number_bytes(32)
        .max_payload_bytes(1024)
        .build();

    let mut ordinary_decoder = JsonDecoder::with_limits(limits);
    let ordinary: serde_json::Value = ordinary_decoder.decode_str(input)?;
    assert_eq!(ordinary["role"], "admin");

    let mut unique_key_decoder = JsonDecoder::with_limits(limits);
    let error = unique_key_decoder
        .decode_unique_str::<DuplicateKeyRejectingJsonValue>(input)
        .expect_err("duplicate object keys must be rejected");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    Ok(())
}
```

The unique-key path provides a safe construction reason through
`JsonDecodeError::materialization_error_reason()`. Read and preserve it before
consuming the error with `into_source()`:

```rust
use qubit_json::decode::{JsonDecodeError, JsonDecodeErrorKind, JsonDecoder};
use qubit_json::decode::JsonDecodeErrorSource;
use qubit_json::decode::JsonMaterializationErrorReason;
use serde_json::Value;

fn main() -> Result<(), JsonDecodeError> {
    let mut decoder = JsonDecoder::unlimited();
    let error = decoder
        .decode_unique_str::<Value>(r#"{"role":"user","role":"admin"}"#)
        .expect_err("duplicate keys must fail materialization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    let reason = error.materialization_error_reason();
    assert_eq!(reason, Some(JsonMaterializationErrorReason::DuplicateKey));
    let source = error.into_source();
    assert!(matches!(source, JsonDecodeErrorSource::Deserialize { .. }));
    Ok(())
}
```

The accessor returns `Some` for duplicate keys or a defensive invalid-number
materialization failure. The legacy `DepthLimit { limit }` reason remains for
source compatibility but is no longer produced by the shared builder.
Admission, input, configured-budget, and target-deserialization failures
return `None`. Reasons contain no key or backend text and remain available
under redacted diagnostics. `into_source()` still returns the existing
`JsonDecodeErrorSource`; adapters that need both values must save the reason
first. Materialization coordinates remain one-based UTF-8 byte columns, and
only LF starts a new line.

With `serde_json`'s `raw_value` feature enabled, ordinary `serde_json::Value`
also gives special meaning to a first object key named
`$serde_json::private::RawValue`. The admitted source
`{"$serde_json::private::RawValue":"null"}` becomes `null`; a non-string
value such as `{"$serde_json::private::RawValue":8}` fails target
deserialization. A first string value containing `[1,2,3]` becomes an array.
These are existing Serde target semantics: admission and resource accounting
describe the source text, while the target may project it into a different
shape. Thus the source object below uses two nodes, although the returned array
contains four values, and `decode_object_str` checks the source root rather than
guaranteeing the projected root remains an object.

```rust
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecoder;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use serde_json::Value;

let text = r#"{"$serde_json::private::RawValue":"[1,2,3]"}"#;
let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
    .max_nodes(2)
    .build();
let mut decoder = JsonDecoder::with_limits(limits);
let projected: Value = decoder.decode_object_str(text)?;
assert_eq!(projected.as_array().unwrap().len(), 3);
assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));

let mut literal_decoder = JsonDecoder::with_limits(limits);
let literal: DuplicateKeyRejectingJsonValue = literal_decoder.decode_unique_str(text)?;
let literal = literal.into_inner();
assert!(literal.is_object());
assert_eq!(literal["$serde_json::private::RawValue"], "[1,2,3]");
# Ok::<(), qubit_json::decode::JsonDecodeError<JsonResource>>(())
```

The unique-key typed entry points materialize a strict tree before invoking the
target, so canonical numeric values become native events even with
`arbitrary_precision`. Their source tree uses the shared builder without a
second fixed depth cap, but the requested target may still impose its own
recursive construction or destruction limits. The intermediate tree and
target may coexist, and iteration order follows `serde_json::Map`. RawValue
targets receive reconstructed JSON rather than the original spelling.
Duplicate-key errors retain source coordinates; configured depth failures are
budget errors, while target-deserialization errors have no original source
position. For deep owned JSON, prefer the protected `decode_value_*` API.
Input charges remain consumed on failure, while value charges commit only when
the entire operation succeeds.

`DuplicateKeyRejectingJsonValue` preserves marker-named objects literally. A
bare `serde_json::Value` target retains its own private-key projection rules,
including when used with the unique-key entry points.

The wrapper validates nested objects recursively and can also be the target of
`NormalizingJsonDecoder` when the configured text cleanup is part of the input
contract. `DuplicateKeyRejectingJsonValueSeed` provides the corresponding seed
for a caller-owned Serde deserializer.

Native `i64`, `u64`, and finite `f64` events retain their numeric
representation. An upstream deserializer with `arbitrary_precision` may expose
a number as map events; the wrapper treats those events as an ordinary object
because it cannot determine their provenance. Ordinary `decode_str` and
`decode_seed_utf8` admission does not restore that provenance. Use the
`decode_unique_*` methods for complete, owned JSON documents. Generic Serde
entry points keep their existing event semantics. See the [JSON number contract](number_contract.md).
JCS uses binary64 and rounds integers beyond its exact range, so it is not a
general replacement when exact integer representation is required.

### Read and mutate existing trees

Use `JsonTreeReader` when the admitted HTTP document must be inspected without
changing it. This standalone visitor counts the root and its descendants under
a separate value budget, then publishes accounting only after traversal succeeds:

```rust
use std::convert::Infallible;
use qubit_budget::json::{JsonResource, JsonValueLimits};
use qubit_json::value::traverse::{JsonTreeContext, JsonTreeReader, JsonTreeVisitor};
use serde_json::{Value, json};

struct CountNodes(usize);

impl JsonTreeVisitor for CountNodes {
    type Error = Infallible;

    fn enter(&mut self, _: &Value, context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        assert!(context.depth >= 1);
        self.0 += 1;
        Ok(())
    }
}

let limits = JsonValueLimits::<JsonResource, usize>::builder()
    .max_depth(8)
    .max_nodes(16)
    .max_payload_bytes(128)
    .build();
let mut budget = limits.budget();
let mut transaction = budget.transaction();
let mut visitor = CountNodes(0);
let value = json!({"id": 42, "ok": true});
JsonTreeReader::new(&mut transaction).process(&value, &mut visitor)?;
transaction.commit()?;
assert_eq!(visitor.0, 3);
assert_eq!(budget.used_nodes(), Some(3));
# Ok::<(), Box<dyn std::error::Error>>(())
```

`enter` runs before descendants; optional `leave` runs after them. Callbacks
receive `JsonTreeContext { depth, location }`: the root has depth one and
`JsonTreeLocation::Root`, while descendants have `ArrayElement { index }` or `ObjectValue { key }`
locations in their immediate parent, not a complete path. For a protected
decoded document, replace `&value` with `owner.as_value()`.

For an existing value, `JsonTreeReader::account` stages complete-tree charges
in the caller's transaction without invoking a visitor. `JsonTreeMutator`
first admits the original tree, then runs in-place visitor callbacks, and
finally admits the mutated tree:

```rust
use std::convert::Infallible;

use qubit_budget::json::JsonValueLimits;
use qubit_json::value::traverse::{
    JsonTreeContext, JsonTreeControl, JsonTreeMutVisitor, JsonTreeMutator,
};
use serde_json::{Value, json};

struct RemoveSecrets;

impl JsonTreeMutVisitor for RemoveSecrets {
    type Error = Infallible;

    fn visit(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        Ok(JsonTreeControl::Descend)
    }
}

let limits = JsonValueLimits::<qubit_budget::json::JsonResource, usize>::builder()
    .max_nodes(32)
    .max_payload_bytes(256)
    .build();
let mut input_budget = limits.budget();
let mut output_budget = limits.budget();
let mut input = input_budget.transaction();
let mut output = output_budget.transaction();
let mut value = json!({"name": "qubit", "secret": "remove me"});

JsonTreeMutator::new(&mut input, &mut output)
    .process(&mut value, &mut RemoveSecrets)
    .expect("both trees fit the configured budgets");
input.commit().expect("input accounting commits");
output.commit().expect("output accounting commits");
assert_eq!(value, json!({"name": "qubit"}));
```

Its errors are
`JsonTreeMutateError::InputBudget`, `::Visitor`, and `::OutputBudget`; visitor
and output failures retain mutations already made to the value. A visitor can
return `JsonTreeControl::SkipSubtree` to skip descendant callbacks, but final
output accounting still covers every resulting descendant. The reader admits
each node before its callback. The mutator admits the complete input first,
then visits nodes (including descendants introduced by earlier callbacks),
then admits the complete result. Output limits do not cap callback count or
intermediate expansion: visitors that add descendants must ensure their own
traversal terminates. `SkipSubtree` skips callback descent at that node; it
does not skip final output admission.

For visitor-based traversal, `JsonTreeReader::process` stops immediately on an
admission or visitor error. Earlier `enter` callbacks are not guaranteed
matching `leave` callbacks, and visitor side effects are not undone. Charges
already staged in the caller's transaction are not rolled back by `process`.
A visitor error alone does not poison that transaction; a budget rejection
prevents a successful commit. For all-or-nothing publication, commit only
after complete success and drop the transaction on failure. Rebuild or restore
visitor state separately.

## Numeric paths, private protocols, and Display

### Text number models

Ordinary text decoding defaults to `JsonDecodeNumberModel::Integer64`: integer
tokens must fit `i64` or `u64`, while decimal and exponent tokens must produce
finite `f64` values. `Integer64WithFiniteFallback`, selected through
`JsonValidatedDecodeOptions::builder().number_model(...)` or
`JsonDecoder::decode_input_with_number_model`, changes admission of only
out-of-range integer tokens. Validated decoding with `JsonNumberPolicy::Preserve`
materializes such a token as finite `f64`, possibly rounded. The typed input
path delegates the result representation to `T` and active `serde_json`
features; with `arbitrary_precision`, it may preserve the wide integer, which
strict re-encoding then rejects as out of range. Both ordinary models still
reject `1e400`. `JcsDecoder` uses binary64 for every number, including in-range
integers, and also rejects non-finite results. See the [number
contract](number_contract.md) for runnable examples.

Choose one of three numeric boundaries deliberately:

- **Ordinary text and Serde events:** the default `Integer64` strict text model requires negative integers in `i64`, non-negative integers in `u64`, and fractional or exponent values that convert to finite `f64`. Explicit fallback admits wider integer tokens when they convert to finite `f64`. Validated `Preserve` materializes those tokens as finite `f64`; typed input decoding delegates representation to `T` and active Serde features, so a preserved wide integer may fail strict re-encoding. `i128` and `u128` serializer/seed events remain explicitly range-checked. Text decoding checks the original-token `NumberBytes` before numeric range. `AccountingJsonValueSeed` cannot measure that token, but enforces `NumberBytes` using the compact display length of the constructed `Number`; by default, `1e2` (3 source bytes) displays as `100.0` (5 bytes). See the [number contract](number_contract.md). `DuplicateKeyRejectingJsonValueSeed` does not apply a resource budget. For map-shaped Serde events, a seed also cannot tell a real object from an upstream imitation of serde_json's private Number map.
- **An already materialized `Value` tree:** `JsonTreeReader` counts each existing Number's actual compact display representation without converting it to `f64` or normalizing it. With `arbitrary_precision`, it can count a stored `1e400` or a 39-digit integer at its actual representation length. This accounting rule does not make the number acceptable to an ordinary or JCS encoder.
- **JCS text:** `JcsDecoder` uses finite IEEE-754 binary64 for every token, including integer lexemes. For example, `9007199254740993` rounds to `9007199254740992`; overflow such as `1e400` is rejected. It also rejects keys duplicated after JSON unescaping and uses the shared builder with configured `max_depth`.

The encoder recognizes two exact serde_json private struct protocols: `RawValue` and `Number`. A genuine Number protocol contains one string payload, accepted through `serialize_str` or one `collect_str`; the text is validated before emission or Value construction. A normal map containing a key named `$serde_json::private::Number` remains an object. Invalid hand-written protocol shapes produce `JsonSerializationErrorKind::InvalidSerializerState` with `JsonSerializerStateError::InvalidRawValueProtocol` or `InvalidNumberProtocol`. Diagnostics do not retain the private payload text.

Newtype structs are serialized transparently by `JsonValueEncoder`,
regardless of their name. A custom newtype named
`$serde_json::private::RawValue` keeps its string, boolean, or `Option` payload
as that ordinary value; it is not parsed again as JSON. Use a genuine
`serde_json::value::RawValue` when raw JSON text must be materialized.
`JsonEncoder` rejects a custom serializer that declares an ordinary compound
length of zero and then emits an element or field, with
`InvalidSerializerState`. `JsonValueEncoder` uses ordinary compound lengths as
preallocation hints and does not enforce the same declaration check. Its
private `RawValue` and `Number` protocols remain strictly validated.

For a `Display` value encoded as a JSON string or object key, each formatting chunk is checked against the prospective complete string/key measurement before the collector appends it. A successful preview does not consume budget; after formatting, the complete measurement is admitted once. This prevents the library collector from growing past configured string/key limits. It cannot control allocations made internally by user `Display::fmt`, allocator metadata, or process RSS. For a `RawValue`/Number raw-output payload, the JSON structure is validated only after the complete text has been collected; output-byte bounds still apply during collection.

## Errors and diagnostics

Strict and normalizing decoders return the same generic `JsonDecodeError`.
Branch on the stable category returned by `kind()`:

| `JsonDecodeErrorKind` | Meaning | Structured details |
| --- | --- | --- |
| `Budget` | A configured resource limit rejected a measurement | `budget_error()`, `raw_input_bytes()`, `normalized_input_bytes()` |
| `EmptyInput` | Configured normalization produced no JSON document | `stage()`, input byte counts |
| `InvalidUtf8` | A byte input was not valid UTF-8 | `utf8_valid_up_to()`, `utf8_error_len()` |
| `InvalidJson` | Syntax or the numeric contract was invalid | `syntax_error()`, `line()`, `column()` |
| `UnexpectedTopLevel` | An object/array-specific API received the wrong root kind | `expected_top_level()`, `actual_top_level()` |
| `Deserialize` | An admitted document could not materialize as the requested type | `line()`, `column()`, unique-key `materialization_error_reason()`, and a detailed source when enabled |
| `Read` | The input source failed before document decoding completed | `read_error_kind()`, `raw_input_bytes()`, and `read_error()` under detailed diagnostics |

For `InvalidJson`, `line()` and `column()` use the lexical scanner's one-based
coordinates: columns count UTF-8 characters, and the byte-based location is
available from `syntax_error().offset()`. LF, CRLF, and standalone CR each
start a new line; CRLF counts as one line break. For `Deserialize`, the
coordinates preserve `serde_json`'s one-based line and byte column, and a zero
backend coordinate is exposed as `None`. Unique-key materialization counts
bytes before the offending token, and only LF starts a new line. Target-seed
errors do not have an original source position. Normalizing decoders report
positions in normalized text and do not map them back to the original input.
Under `Detailed` diagnostics, a retained parser source can format coordinates
according to its backend, so its text coordinates need not match the
structured scanner coordinates.

`JsonSyntaxError::context()` adds the grammar location when the scanner knows
it, such as `ObjectSeparator` or `Value`. This field participates in derived
`Eq` and `Debug`, so those structured representations can differ from earlier
versions; `Display` remains unchanged and does not print the context. The
context is propagated through validated errors in both redacted and detailed
diagnostic modes. `Some(context)` identifies the active grammar position:
`Value`, `ObjectKey`, `String`, `Number`, `Colon`, `ArraySeparator`,
`ObjectSeparator`, or `TrailingInput`. `None` means no scanner grammar position
was recorded; for example, `JsonSyntaxError::new` starts without a context.
`with_context` returns the same error with a supplied context and preserves its
reason and source coordinates. When handling a validated `Decode` error, its
`context` field matches `error.syntax_error().and_then(|syntax| syntax.context())`
when that syntax error is present.

Adapters that own the error can instead call `into_source()` and exhaustively
match `JsonDecodeErrorSource` without cloning budget or syntax details.

`JsonValueEncoder` and `JsonEncoder` share the privacy-safe
`JsonSerializationError` model. Use
`JsonSerializationError::category()` for recovery policy and `kind()` when the
exact reason matters. Convenience predicates cover the common groups, while
typed accessors expose only applicable details:

| Category | Representative kinds | Convenience/details |
| --- | --- | --- |
| `Number` | `IntegerOutOfRange`, `NonFiniteFloat`, `InvalidNumberRepresentation` | `is_number_error()`, `integer_signedness()` |
| `ObjectKey` | `UnsupportedMapKey`, `DuplicateObjectKey` | `is_map_key_error()`, `map_key_kind()` |
| `RawValue` | `InvalidRawValue` | `is_raw_value_error()` |
| `Capacity` | `CollectionLengthOverflow` | `collection_kind()` |
| `SerializerContract` | `InvalidSerializerState` (including `InvalidRawValueProtocol` and `InvalidNumberProtocol`), `DisplayFormattingFailed` | `is_serializer_contract_error()`, `serializer_state_error()` |
| `Custom` | `CustomSerialization` | Opaque by design: arbitrary serializer text is not retained |

`stage()` identifies the public processing boundary precisely:

| `JsonDecodeStage` | Boundary |
| --- | --- |
| `Input` | Charge raw input bytes |
| `DecodeText` | Validate byte input as UTF-8 |
| `Normalize` | Transform text or charge normalized bytes |
| `Admission` | Admit decoded-value resources |
| `Parse` | Validate JSON syntax and numeric range |
| `TopLevelCheck` | Enforce an object or array root |
| `Deserialize` | Materialize the requested Rust type |

Redacted diagnostics retain stable classifications and source coordinates, but
never an offending byte, token/key/value text, or a parser/Serde source.
Input-derived sources are retained only by `DiagnosticPolicy::Detailed`.
Configure strict decoding with
`JsonDecoder::with_diagnostic_policy(DiagnosticPolicy::Detailed)`; configure
normalizing decoding through `NormalizingJsonDecodePolicyBuilder`. Enable
detailed diagnostics only at trusted boundaries and keep untrusted logs
redacted. If a trusted caller needs the exact byte, it should inspect its
original input using the stable offset; the error does not retain it. Encoding
never retains arbitrary third-party `Serialize::custom` text. The other
domains expose `JsonEncodeError`, `JsonSyntaxError`, `JsonTreeProcessError`,
and `JsonTreeMutateError`.

This redaction boundary covers diagnostics generated by the library. A
`JsonValidatedDecodeError::Policy` preserves the error returned by the caller's
number policy, and `Error::source()` exposes it when the error implements
`Error + 'static`. The outer validated error omits that value from its own
`Display` and `Debug`, but the source chain can still contain sensitive text.
`DiagnosticPolicy::Redacted` does not sanitize caller-provided errors. If a
policy error includes `token.raw()` or other sensitive data, the caller must
choose safe error contents and control how the source chain is logged. See the
[`JsonValidatedDecodeError` API documentation](https://docs.rs/qubit-json/latest/qubit_json/decode/enum.JsonValidatedDecodeError.html)
for an example.

The default `Integer64` numeric contract is independent from resource limits:
negative integers fit `i64`, non-negative integers fit `u64`, and
fractional/exponential values must be finite `f64`. Under this default model,
values outside those ranges or exact decimals that must avoid binary rounding
should use a string or explicit domain representation. Explicit finite
fallback can admit wider integer tokens when they convert to finite `f64`, with
possible rounding. See the [number contract](number_contract.md).

## Troubleshooting

- A `Budget` error: inspect the matching limit (input bytes, number bytes,
  depth, nodes, collection sizes, key/string bytes, or output bytes) and keep
  the session alive only if cumulative accounting is intended.
- An `EmptyInput` error from `NormalizingJsonDecoder`: check whether configured
  whitespace, BOM, or fence handling removed the complete input. A raw empty
  input passed to strict `JsonDecoder` is classified as `InvalidJson`.
- An `InvalidUtf8` error: inspect `utf8_valid_up_to()` and `utf8_error_len()`,
  and reject or repair the byte transport before JSON parsing.
- An `InvalidJson` error: validate the original bytes and check the reported reason,
  offset, line, and column; normalization never invents missing JSON syntax.
- A `Read` error: distinguish an input-source failure from invalid JSON syntax.
  Bytes already accepted remain charged, and the source is not rewound
  automatically. `read_error_kind()` and `raw_input_bytes()` are available in
  redacted diagnostics; the original source is available only with Detailed
  diagnostics. Retry only when the input source can safely resume or rewind.
- An `UnexpectedTopLevel` error: compare `expected_top_level()` with
  `actual_top_level()`, then either fix the payload or select a decoder method
  whose root contract matches it.
- A `Deserialize` error: first inspect
  `materialization_error_reason()`. `DuplicateKey` identifies repeated object
  keys (see [Duplicate object keys](#duplicate-object-keys)); the legacy
  `DepthLimit { limit }` reason remains for source compatibility but is not
  produced by the shared builder; configured `max_depth` failures use `Budget`;
  `InvalidNumber` identifies a defensive number-materialization failure. Save
  this reason before calling `into_source()`. If it is `None`,
  then investigate the target schema, custom seed, or Serde construction
  protection. Typed-deserialization behavior is separate from JCS Value
  construction. `JcsDecodeError::MaterializationDepthLimit` remains a
  compatibility variant but is not produced by the shared builder; JCS depth
  budget failures are wrapped in `Decode`. Handle `Decode` and `DuplicateKey`
  explicitly; duplicate keys include the relevant source line and column.
- An unexpected tree mutation after an error: `JsonTreeMutator` is incremental;
  visitor and output-budget failures do not roll back prior mutations.

## Limitations and best practices

Set finite limits at every untrusted boundary, including input/output bytes,
depth, nodes, collection sizes, key/string bytes, and number bytes. Do not use
an unlimited session merely to avoid choosing limits. Keep application
validation separate from resource admission, and treat browser `BigInt`
handling as a wire-format concern for integers above JavaScript's safe range.
Keep Serde's depth guard enabled as well. Disabling it only moves deeply nested
untrusted inputs onto the Rust call stack and does not make arbitrary target
deserializers safe.

## Further reading

- [English README](../README.md) · [中文 README](../README.zh_CN.md)
- [中文用户手册](user_guide.zh_CN.md)
- [JSON number contract](number_contract.md)
- [Released API documentation](https://docs.rs/qubit-json); generate the
  current branch API with `cargo doc --all-features --open`
