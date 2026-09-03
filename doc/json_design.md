# `qubit-json` 0.8 design

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
`DiagnosticPolicy::Redacted` is the default. Strict decoding can opt into detailed diagnostics with
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
not enable serde_json arbitrary precision or recognize its old private Number
marker. See [the number contract](number_contract.md) and [the numeric design
decision](number_contract_design.md).

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

## Related design documents

- [Product requirements](json_prd.md) · [中文产品需求](json_prd.zh_CN.md)
- [Number contract](number_contract.md) · [数字契约](number_contract.zh_CN.md)
- [Number contract design](number_contract_design.md) ·
  [数字收敛设计](number_contract_design.zh_CN.md)
- [Error, performance, and downstream safety](error_performance_redaction_design.md) ·
  [错误、性能与下游安全边界](error_performance_redaction_design.zh_CN.md)
- [Performance evidence log](benchmark_baseline.md) ·
  [性能证据日志](benchmark_baseline.zh_CN.md)
- [Dependency maintenance](dependency_maintenance.md) ·
  [依赖维护](dependency_maintenance.zh_CN.md)
