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
├── lexical/            # crate-private scanner and shared implementation
└── value/
    └── traverse/       # non-recursive reader, mutator, and tracker
```

`lexical` is not a public interface. The crate does not share public APIs
through root-level error or option modules.

## Two facades and one shared core

The public layer keeps two distinct responsibilities: `JsonDecoder` handles
strict JSON without modifying input, while `NormalizingJsonDecoder` handles
explicitly permitted text normalization. Both delegate to the crate-private
`JsonDecodeEngine<'budget, R, Q>`, generic over resource and quantity types.
The shared core owns sessions, lexical admission, the numeric contract,
top-level checks, Serde materialization, and transaction boundaries. The
facades only select whether normalization happens first and shape the public
entry points. This avoids conflating trust boundaries or maintaining two
error and accounting implementations.

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

### Numeric representation

Strict and normalizing text paths share one numeric contract: negative
integers fit `i64`, non-negative integers fit `u64`, and fractional or
exponential values must be finite `f64`. The lexical scanner admits
`NumberBytes` before range validation, so resource and representation limits
remain separate and budget failures have priority. The implementation does
not enable serde_json arbitrary precision or recognize its private Number
marker. See [the number contract](number_contract.md).

This keeps object structure and budget categories consistent: an object with
`$serde_json::private::Number` as a key remains an ordinary object rather than
becoming a number through a private Serde protocol. The separate `RawValue`
integration remains supported. Finite float lexeme lengths use the public
`serde_json::ser::CompactFormatter` interface, without a direct dependency on
its underlying float-formatting library.

## Value

`AccountingJsonValueSeed` is the public seed for constructing
`serde_json::Value` from Serde events. The caller binds it to a
`JsonValueTransaction`; it is useful when the original JSON bytes are no
longer available but the materialized value still needs accounting. Lexical
admission and the seed have different responsibilities: the seed cannot
inspect the original number lexeme, so text-level number and `NumberBytes`
guarantees must come from `JsonDecoder`.

## Tree

`JsonTreeReader` performs depth-first enter/leave traversal over an immutable
`Value`; `JsonTreeMutator` performs visitor-driven, non-recursive traversal
over a mutable `Value`. Both admit nodes, containers, strings, numbers, and
object keys before callbacks.

`JsonTreeReader::account` stages complete-tree charges in an existing caller
transaction without invoking a visitor or creating and committing another
transaction. `JsonTreeBudgetTracker` commits that path after success and
provides an owned, resettable budget for materialized trees.

`JsonTreeMutator` first admits the original tree, then runs visitor-driven
in-place mutations, and finally admits the complete result. It returns
`JsonTreeMutateError::InputBudget`, `::Visitor`, or `::OutputBudget`.
Visitor and output failures retain mutations already performed.
`JsonTreeControl::SkipSubtree` skips descendant callbacks only; final output
accounting still covers every resulting descendant.

## Public error model

Public errors are divided by domain:

1. `decode::JsonDecodeError` and its owned `JsonDecodeErrorSource`, shared by
   both decoder facades.
2. `encode::JsonEncodeError`.
3. `decode::JsonSyntaxError`.
4. `value::traverse::JsonTreeProcessError`.
5. `value::traverse::JsonTreeMutateError`.

Each error exposes only context that its domain can provide stably. There is
no root-level error aggregate or compatibility alias.

## Serialization errors and output boundaries

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
allocator metadata, and output-buffer capacity. The configured value limits
such as `max_depth`, `max_map_entries`, and `max_nodes` remain independent;
`max_output_bytes` limits canonical output bytes, not renderer working memory.

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
- The private Number marker is an ordinary key. Numeric lexical budgets take
  precedence over range checks.
- Tree traversal avoids Rust recursion. Budget and visitor failures preserve
  their documented commit and mutation boundaries.
- Default diagnostics retain no input content; custom serialization errors
  never expose arbitrary source text.
- Successful benchmark paths assert fixture admission, so cumulative budget
  exhaustion cannot silently turn them into failure-path measurements.
- Differential fuzzing separates lexical validation from target construction.
  Deep admitted input may hit Serde's construction recursion guard; direct
  materialized-value comparison is limited to conservatively shallow input.
  Oversized fuzz inputs are rejected in full rather than truncated for comparison.

## Related documents

- [User guide](user_guide.md) · [用户手册](user_guide.zh_CN.md)
- [Number contract](number_contract.md) · [数字契约](number_contract.zh_CN.md)
- [Benchmark guide](benchmark_baseline.md) · [基准测试指南](benchmark_baseline.zh_CN.md)
