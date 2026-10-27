# `qubit-json` design

[中文版](json_design.zh_CN.md)

## Goals and boundaries

`qubit-json` provides resource-aware infrastructure for JSON input, text,
values, and trees. It does not define another JSON data model or replace
Serde's typed deserialization. `qubit-budget` owns limits, resource
identities, budgets, and sessions; this crate implements JSON-specific
behavior.

The public source layout is:

```text
src/
├── decode/             # normalization, strict decoding, and errors
├── encode/             # strict stateful text encoder
├── format/             # syntax-checked token-preserving formatting
├── lexical/            # crate-private scanner and shared implementation
└── value/
    └── traverse/       # non-recursive reader, mutator, and tracker
```

`lexical` is not a public interface. The crate does not share public APIs
through root-level error or option modules.

## Decode facades and one shared core

The public layer keeps distinct responsibilities: `JsonDecoder` handles
strict JSON without modifying input, `NormalizingJsonDecoder` handles
explicitly permitted text normalization, and feature-gated `JcsDecoder`
handles strict raw text with binary64 number and duplicate-key rules. They
delegate to the crate-private `JsonDecodeEngine<'budget, R, Q>`, generic over
resource and quantity types.
The shared core owns sessions, lexical admission, the numeric contract,
top-level checks, Serde materialization, and transaction boundaries. The
facades only select whether normalization happens first and shape the public
entry points. This avoids conflating trust boundaries or maintaining two
error and accounting implementations.

### Owned unique-key decoding

`JsonDecoder::decode_unique_str`, `decode_unique_utf8`, and
`decode_unique_seed_utf8` require an owned result (`DeserializeOwned`, or a
seed whose output type is independent of the input lifetime). This path first
performs ordinary strict lexical admission and stages the source document's
value accounting. It then scans the bytes a second time to build an
intermediate `serde_json::Value`; the second scan emits synchronous lexical
events and disables a second round of accounting. Thus each source resource is
charged once. The target seed receives Serde events from this owned tree, not
events borrowing the source text.

The intermediate tree uses the strict `Integer64` number model: negative
integers become `i64`, non-negative integers become `u64`, and fractional or
exponential numbers become finite `f64`. Deserializing that tree exposes
native integer and floating-point Serde events, including when
`arbitrary_precision` is enabled. Object keys are compared after JSON
unescaping, without Unicode normalization. The shared builder has no separate
fixed container cap: configured root-inclusive `max_depth` controls depth
admission. The legacy `DepthLimit { limit }` reason remains for source
compatibility; configured depth rejection is currently reported as a budget
error.
The temporary tree and final target can coexist, and the lexical resource
budgets do not represent a precise heap-memory ceiling. Objects use
`serde_json::Map` iteration order.

`RawValue` targets receive reconstructed JSON serialized from the intermediate
tree, rather than a slice of the original text. `serde_json::Value` retains
its own interpretation of the private `$serde_json::private::RawValue` marker;
use `DuplicateKeyRejectingJsonValue` when literal private marker keys must
remain ordinary data. Duplicate-key construction failures are reported as
deserialization errors with one-based source coordinates at the second key's
opening quote. Configured depth failures are budget errors from admission.
Target seed failures do not have original source coordinates. Value accounting commits only after
target construction succeeds; accepted input charges remain consumed on
failure. See the [duplicate object keys guide](user_guide.md#duplicate-object-keys).

`JsonDecodeError::materialization_error_reason()` adds a copyable side-channel
reason for unique-key construction failures: `DuplicateKey` or defensive
`InvalidNumber`. The legacy `DepthLimit { limit }` reason remains for source
compatibility but the shared builder does not currently produce it. Other
decode failures return `None`. The reason is independent of `DiagnosticPolicy`, contains no
input text, and is preserved by `Clone`. Read it before `into_source()`;
`JsonDecodeErrorSource` and its public variants remain unchanged, so existing
coarse error mappings remain compatible. Materialization coordinates retain
one-based UTF-8 byte columns and LF line breaks.

## Normalizing facade

`NormalizingJsonDecoder` holds an immutable `NormalizingJsonDecodePolicy` and
only applies explicitly configured rules: whitespace, BOM, Markdown fences,
and control characters inside strings. It never guesses missing JSON
punctuation or structure.

The policy defines normalization and diagnostic behavior, not budgets. The
caller supplies limits with
`NormalizingJsonDecoder::with_limits(policy, limits)`, or reuses a
`JsonDecodeSession` with `NormalizingJsonDecoder::new(policy, session)`.
Raw and normalized input bytes, depth, nodes, collection sizes, and payload
limits all come from that limits object or session. A default limits value is
unlimited only when the caller explicitly selects it. Raw and normalized input
are charged first; decoded-value charges remain staged until complete typed
decoding succeeds. Input charges remain in the session after a failed decode.

One-shot `decode_str` and `decode_utf8` require `DeserializeOwned`, so a
temporary normalized buffer cannot escape the call. For borrowed decoding,
Serde seeds, or repeated materialization, call `prepare_str` or
`prepare_utf8` and then decode the resulting `NormalizedJsonDocument`.
Preparation commits raw and normalized input charges once; each document
decode stages and commits its own value charge. The document is not tied to
the decoder that created it. If normalization needs no allocation, its text
may borrow the original input; escaped strings still require owned targets.

Both decoders consume complete `&str` or `&[u8]` inputs. Their input-byte
limits do not retroactively cap memory already allocated by an outer transport
or body aggregator; a bounded read or aggregation limit belongs at that outer
boundary.

`JsonDecoder::decode_input` instead reads an `Input<Item = u8>` source through
EOF into an owned result. With a finite input budget, each read is limited to
the remaining budget plus at most one probe byte. The probe distinguishes EOF
at the exact limit from additional input; it is not charged or buffered, and
`Input` cannot return it to the source. Any prefix within the budget is charged
and retained even when the probe causes a budget error.

## Strict text

Strict text APIs are object-based so state does not spread through stateless
free functions:

```rust
use qubit_budget::json::{JsonDecodeLimits, JsonResource};
use qubit_json::decode::JsonDecoder;

let mut decoder = JsonDecoder::with_limits(
    JsonDecodeLimits::<JsonResource, usize>::new(),
);
let value: serde_json::Value = decoder.decode_utf8(br#"{"ok":true}"#)?;
# Ok::<(), qubit_json::decode::JsonDecodeError<JsonResource>>(())
```

`JsonDecoder` owns a `JsonDecodeSession`. `with_limits(limits)` is the usual
explicit constructor; `new(session)` is for caller-managed reuse. Use
`unlimited()` only when unlimited accounting is intentional. The codec does
not implement `Default`, so unlimited accounting is not disguised as a safe
default. The decoder provides string, UTF-8, seed, and validation entry
points. Each attempt records input, runs the shared scanner, and commits the
value transaction only after complete success.

`JsonEncoder` owns a `JsonEncodeSession` and provides `to_vec`,
`write_buffered`, and `write_incremental`. Buffered writes touch the target
only after the complete byte sequence is available; incremental writes may
leave an accepted prefix after failure.

Strict and normalizing decoding return the same generic
`JsonDecodeError<R, Q>`. Callers use `JsonDecodeErrorKind`,
`JsonDecodeStage`, and budget, syntax, UTF-8, and top-level accessors rather
than parsing messages. Callers that own the error can use `into_source()` to
exhaustively match `JsonDecodeErrorSource` without cloning structured data.
`DiagnosticPolicy::Redacted` is the default. It retains only stable error
classification and source coordinates, never an offending byte or other input
content. Strict decoding can opt into detailed diagnostics with
`with_diagnostic_policy`; normalizing decoding takes the setting from its
policy. Strict encoding returns `JsonEncodeError` with budget, invalid raw
JSON, serialization, or write failures. `JsonSyntaxError` separately owns a
stable reason and source position.

`JcsDecoder` returns `JcsDecodeError<R, Q>`, with a `Decode` case wrapping the
existing decode error, a structured `DuplicateKey { line, column }` case, and
`MaterializationDepthLimit { limit, line, column }`. The materialization errors
store no key text or backend source. After complete lexical admission, the
scanner runs again and synchronously sends events to an iterative Value builder;
duplicate keys are detected before inserting their values. The shared builder
has no separate fixed container cap; root-inclusive configured `max_depth`
controls admission and depth rejection is reported as a budget error. The
historical `MaterializationDepthLimit` variant remains for source compatibility
but is not produced by the current builder. Unrelated syntax and budget errors keep their ordinary
decode classification. Value accounting commits only after the complete
document succeeds, while accepted input bytes remain charged after failure.
The shared RawValue materializer used by `JsonValueEncoder` applies the same
configured depth contract to genuine `serde_json::value::RawValue` payloads. Its
`InvalidRawValue` error identifies rejection by the strict materialization
contract, including duplicate keys, numeric constraints, or depth; it is not
a syntax-only classification.

### Numeric representation

Strict and normalizing text paths default to `Integer64`: negative integers
fit `i64`, non-negative integers fit `u64`, and fractional or exponential
values must be finite `f64`. The opt-in `Integer64WithFiniteFallback` model
allows an out-of-range integer token when it converts to finite `f64`, which
can round its value; it does not relax the finite rule for decimal or exponent
tokens. The feature-gated `JcsDecoder` uses a separate binary64 model,
converting every number (including in-range integers) to `f64` and rejecting
duplicate object keys during materialization. Both ordinary models admit the
original `NumberBytes` before range validation, so budget failures have
priority. The crate does not enable serde_json `arbitrary_precision` itself,
but it handles the exact private `Number` struct protocol when another
dependency enables it. A normal map with a matching key remains an object. See
[the number contract](number_contract.md) for the model comparison and
configuration examples.

`JcsDecoder` reuses lexical admission, sessions, and transactions. It performs a
complete first scan for admission and accounting, then scans again and builds
the Value synchronously from lexical events without collecting a document-sized
event array. The builder checks decoded key strings before insertion and does
not normalize Unicode. JCS Value construction uses the shared builder and has
no independent fixed container cap. Root-inclusive configured `max_depth`
controls admission; depth rejection is returned as a budget error wrapped in
`Decode`. The legacy `JcsDecodeError::MaterializationDepthLimit { limit, line,
column }` variant remains for source compatibility but is not produced by
this builder. `decode_input` keeps the
read-through-EOF, full-buffering behavior and may consume one uncharged probe
byte with a finite input limit. Budgets describe measured resources, not an
exact malloc ceiling. JCS decoding parses and constructs a value; verifying
already canonical bytes requires callers to compare input with encoded output.

The private Number protocol is recognized only by its exact Serde struct name and payload shape. A real object whose key is `$serde_json::private::Number` remains an object. The separate `RawValue` protocol is also recognized and strictly validated. Finite float lexeme lengths use the public
`serde_json::ser::CompactFormatter` interface, without a direct dependency on
its underlying float-formatting library.

## Protected values and Serde bridges

`SafeJsonValue` owns a `serde_json::Value` and performs its own copy, equality,
and cleanup through guarded iterative operations. This protects those owner
operations from recursive tree depth; it does not impose an exact heap limit
or change the recursion behavior of a caller's Serde target.

Its generic `Deserialize` implementation consumes Serde events and protects
partially built values, but it has no source-token provenance. In particular,
a map-shaped event cannot be identified as either a literal object or
`serde_json`'s private arbitrary-precision number protocol. Such events remain
ordinary objects, so direct bridge results for numeric text may differ when a
dependency enables `arbitrary_precision`. Use
`JsonDecoder::decode_value_str` or its UTF-8/reader counterparts when decoding
JSON text with the text decoder's numeric model. That path recognizes the
private struct protocol while retaining literal objects with the same key.

The direct bridge rejects duplicate object keys after decoding escapes and
does not normalize Unicode. Its duplicate-key index is an auxiliary
`BTreeSet<String>` alongside the collected members. The number of key
comparisons is O(n log n), while each `String` comparison's byte cost depends
on its shared prefix. The temporary key storage is additional to value
accounting. Object assembly continues through `SafeValueBuilder` and follows
the active `serde_json::Map` ordering policy.

## Token-preserving formatting

The public `format` domain exposes `format_json_tokens` and
`write_json_tokens`. It validates one complete JSON document and plans only
whitespace edits; original strings and number tokens, including duplicate
keys, remain byte-for-byte intact. It does not materialize a `Value`, apply a
numeric model, or canonicalize keys. `JsonPrettyOptions` configures indentation
and final newline behavior. `JsonPrettyError` classifies syntax, output-limit,
capacity, and writer failures through `JsonPrettyErrorKind`.

Planning must finish before rendering begins, so syntax and output-limit
failures leave a writer untouched. A bounded planner continues scanning and
checking the complete input after the formatted length exceeds the limit, which
preserves a later syntax error's precedence. It stops saving additional
rendering actions after that point but retains the already collected prefix;
the action array therefore does not grow with the remaining wide input. Input
scan time still scales with input size, and the container stack scales with
nesting depth. The formatted-output maximum is not an auxiliary-memory or
process-RSS limit. A real writer failure can leave an accepted output prefix.

## Value

`AccountingJsonValueSeed` is the public seed for constructing
`serde_json::Value` from Serde events. The caller binds it to a
`JsonValueTransaction`; it is useful when the original JSON bytes are no
longer available but the materialized value still needs accounting. Lexical
admission and the seed have different responsibilities: the seed cannot
inspect the original number lexeme, so text-level `NumberBytes` guarantees must
come from `JsonDecoder`, or from `JcsDecoder` when its JCS number contract is
required. Native integer events are still checked against the signed/unsigned
64-bit union. A map event cannot be distinguished from a private Number payload
without token provenance, so it remains an ordinary object.

`JsonValidatedDecodeError` distinguishes decode failures, caller number-policy
rejections, exact-safe-integer conversion failures, and additional validated
limits. Policy diagnostics retain the source byte offset without formatting
the opaque caller error payload. `JsonValueEncodeError` separates budget
rejection from strict JSON serialization failure when projecting a Serde value
into a caller-owned transaction.

## Tree

`JsonTreeReader` counts an existing `serde_json::Number` using the byte length of its current compact display. This avoids conversion to `f64`, preserves the stored representation, and permits accounting for wide values under `arbitrary_precision`; it does not imply that ordinary or JCS encoding accepts those values.

`JsonTreeReader` performs depth-first enter/leave traversal over an immutable
`Value`; `JsonTreeMutator` performs visitor-driven, non-recursive traversal
over a mutable `Value`. The reader admits each node and its measured fields
before the matching callback. The mutator admits the complete input tree
before its first callback and admits the complete result after traversal.

`JsonTreeReader::account` stages complete-tree charges in an existing caller
transaction without invoking a visitor or creating and committing another
transaction. `JsonTreeBudgetTracker` commits that path after success and
provides an owned, resettable budget for materialized trees.

`JsonTreeMutator` first admits the original tree, then runs visitor-driven
in-place mutations, and finally admits the complete result. It returns
`JsonTreeMutateError::InputBudget`, `::Visitor`, or `::OutputBudget`.
Visitor and output failures retain mutations already performed.
`JsonTreeControl::SkipSubtree` skips descendant callbacks only; final output
accounting still covers every resulting descendant. Unlike the reader's
per-node admission before callbacks, the mutator's output admission happens
after all callbacks. A visitor that creates descendants is responsible for
bounding its own callback work and ensuring that traversal terminates.

## Public error model

Public errors are divided by domain:

1. `decode::JsonDecodeError` and its owned `JsonDecodeErrorSource`, shared by
   the ordinary and normalizing decoders; JCS decoding uses `JcsDecodeError`.
2. `decode::JsonValidatedDecodeError`, for decode, policy, exact-integer, and
   additional-limit failures with source offsets.
3. `encode::JsonEncodeError`.
4. `decode::JsonSyntaxError`.
5. `value::traverse::JsonTreeProcessError`.
6. `value::traverse::JsonTreeMutateError`.
7. `value::JsonValueEncodeError`, which separates projection budget rejection
   from strict JSON serialization failure.
8. `format::JsonPrettyError`, whose kind distinguishes syntax, output limit,
   capacity, and I/O failures.
9. Feature-gated `decode::JcsDecodeError` and `encode::JcsEncodeError`.
10. `encode::JsonSerializationError`, shared by JSON encoders.

Each error exposes only context that its domain can provide stably. There is
no root-level error aggregate or compatibility alias.

## Serialization errors and output boundaries

The exact `RawValue` and `Number` private protocols are state-checked. Their malformed shapes map to `InvalidSerializerState` with `InvalidRawValueProtocol` or `InvalidNumberProtocol`; output diagnostics omit private payload contents. `Display` string/key chunks undergo a prospective measurement check before being appended, then the complete string/key is admitted once. This bounds collector growth by configured logical limits, but cannot constrain allocations inside user `Display::fmt`, allocator metadata, or RSS. Raw JSON payload structure is still checked after its complete text has been collected.


`JsonEncoder` and `JsonValueEncoder` share `JsonSerializationError`. Callers
inspect `kind()` for precise causes and `category()` for recovery policies:
`Number`, `ObjectKey`, `RawValue`, `Capacity`, `SerializerContract`, or `Custom`.
Detail accessors expose only non-sensitive integer signedness, key shapes,
collection kinds, or serializer states. Arbitrary third-party error text maps
to `CustomSerialization` and never enters public Display, Debug, or error
source chains. `JsonEncodeError::into_source()` transfers ownership of the
operation-level source. See the [user guide](user_guide.md) for error categories.

`RawValue` must be fully validated before its bytes reach an external target.
Buffered writing completes serialization and budget checks before touching the
target, but I/O failure can still leave partial bytes. Incremental writing also
cannot roll back external output. Optimizations must preserve numeric ranges,
duplicate-key checks, budget commits, error ordering, and partial-output behavior.

## JCS encoding and working-memory boundary

With the optional `jcs` feature, `JcsEncoder` accepts an already constructed
`serde_json::Value`. `with_limits(limits, max_working_bytes)` and
`new(session, max_working_bytes)` require a separate auxiliary-memory limit;
`max_working_bytes()` reports that configuration. `unlimited()` explicitly
removes value, output, and working-memory limits and is intended only for
trusted or separately admitted values.

The renderer walks arrays and objects iteratively. For each object it sorts
borrowed member references by UTF-16 key order, then writes punctuation and
scalar values directly to the output. It accounts the reserved traversal-stack
and active member-table capacities against `max_working_bytes` before growing
them. This logical capacity bound excludes the caller-owned input `Value`,
allocator metadata, output-buffer capacity, and the admission traversal stack
used before rendering when value limits are configured. Configure `max_depth`
to bound that admission pass and `max_nodes` to bound admitted values. This
capacity accounting does not promise an exact heap limit. `max_output_bytes`
limits canonical output bytes, not renderer working memory.

Value accounting is staged before rendering and commits only after the whole
encoding succeeds. `Budget` reports value or output accounting failures;
`WorkingMemory` reports the auxiliary limit; allocation failure maps to a safe
serialization error. `write_buffered` renders and checks the document before
touching the destination. `write_incremental` charges accepted bytes as they
are written and flushes before reporting success. Either writer may leave an
accepted prefix after a write failure; those bytes remain charged, while
failed operations do not commit value accounting. Incremental writing may
also leave its accepted prefix when its final flush fails. Error precedence
follows the first failure encountered in these stages.

## Resource measurement and performance validation

When limits are disabled, work used only for measurement and admission can be
skipped; syntax, numeric, and key validation remain mandatory.
`JsonValueTransaction::has_limits()` lets readers skip unlimited budget
measurement. Mutators select input and output measurement independently.
Owned-buffer encoding writes directly to `Vec<u8>` when output is unlimited.

Benchmarks distinguish strict validation, value budgets, output budgets, full
budgets, fresh and reused sessions, and buffered and incremental writing. They
cover numbers, strings, objects, `RawValue`, and different tree shapes. See the
[benchmark guide](benchmark_baseline.md) for comparison methods.

## Core invariants and validation boundaries

- Normalization policy and resource limits remain independent. Untrusted input
  needs limits at both the transport and decoding boundaries.
- Failed decoding retains input charges; value charges commit only after full
  success. Preparation charges input once.
- A genuine private Number struct is parsed as a number; an ordinary map key with the same spelling stays an object. Numeric lexical budgets take precedence over range checks.
- Tree traversal avoids Rust recursion. Budget and visitor failures preserve
  their documented commit and mutation boundaries.
- Default diagnostics retain no input content; custom serialization errors
  never expose arbitrary source text.
- Successful benchmark paths assert fixture admission, so cumulative budget
  exhaustion cannot silently turn them into failure-path measurements.
- Differential fuzzing separates lexical validation from target construction.
  Ordinary typed decoding may encounter Serde's construction recursion guard;
  JCS Value construction is iterative and uses the configured root-inclusive
  depth limit, with no separate fixed container cap. Direct materialized-value comparison is limited to
  conservatively shallow input. Oversized fuzz inputs are rejected in full
  rather than truncated for comparison.

## Related documents

- [User guide](user_guide.md) · [用户手册](user_guide.zh_CN.md)
- [Number contract](number_contract.md) · [数字契约](number_contract.zh_CN.md)
- [Benchmark guide](benchmark_baseline.md) · [基准测试指南](benchmark_baseline.zh_CN.md)
