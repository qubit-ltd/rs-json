# JSON error, performance, and downstream safety boundaries

[中文版](error_performance_redaction_design.zh_CN.md)

This document records the design decisions for the post-`qubit-json 0.8`
breaking-change window. It covers value-encoding errors and hot paths in
`rs-json`, as well as contracts consumed directly by `rs-budget`, `rs-value`,
and `rs-redact`.

## Design principles

- Programs handle structured enums rather than strings. Public diagnostics do
  not carry input values, object keys, or third-party `Error::custom` text.
- Strict numbers, duplicate keys, `RawValue` validation, budget transactions,
  and incremental-writer partial-output behavior take priority over throughput;
  an optimization may remove only work that cannot change results.
- `unlimited()` is for trusted input or input admitted by another layer.
  Untrusted input must have both input-byte and structural bounds before a
  complete tree is allocated.
- Experimental implementations are kept only when the target scenario has a
  stable improvement of at least 5% and primary existing scenarios do not
  regress unstably by more than 3%.

## Value-encoding error model

`JsonValueEncodeError` contains only `JsonValueEncodeErrorKind`. The precise
kinds cover:

- signed or unsigned integer overflow, non-finite floats, and invalid number
  representations;
- unsupported Serde map-key shapes and normalized duplicate object keys;
- invalid `RawValue`;
- array/object length overflow;
- invalid compound-serializer order or forged `RawValue` protocols;
- `collect_str` formatting failure; and
- custom errors from an external serializer.

Coarse categories are `Number`, `ObjectKey`, `RawValue`, `Capacity`,
`SerializerContract`, and `Custom`. Downstream code can enumerate `kind()` or
choose a policy from `category()`. Convenience accessors expose only safe
details such as signedness, key shape, collection kind, or serializer-state
reason. The enums are intentionally not `non_exhaustive`; a new variant is an
explicit breaking change.

## Performance model and experiment boundary

Strict encoding necessarily traverses Serde values, checks numbers and keys,
and generates output. Resource measurement should be paid only when a limit
can change the result. The benchmark matrix distinguishes `serde_json`,
strict-only, value-only, output-only, full, owned/reused sessions,
incremental writers, and numeric, string, object, and `RawValue` shapes.

The current experiment decision is:

1. An owned-buffer path without an output limit writes directly into its
   `Vec<u8>` while retaining value accounting and `RawValue` error propagation;
   this optimization met its threshold and remains.
2. After that optimization, strict-only is close enough to direct
   `serde_json` that a second serializer is not justified by a safe small
   change.
3. An operation-local admission plan is considered only if profiling clearly
   points at `rs-budget::try_admit`. Benchmark comparisons alone do not justify
   adding that complexity.
4. `RawValue` must be fully validated before external output can be safely
   committed. Coupling scanner and output-copy state would increase state and
   partial-output risk, so it is not adopted without stronger evidence.

See the [benchmark baseline](benchmark_baseline.zh_CN.md) for reproducible
measurements. Benchmarks describe same-machine facts and do not promise
cross-machine ratios.

## Tree fast path

`JsonValueTransaction::has_limits()` is a read-only query that does not expose
internal state. When the value transaction is unlimited, the reader performs
only traversal and visitor callbacks; the mutator independently decides
whether input and output accounting are needed. `tree_bench` reports large
arrays, large objects, and deep trees separately for visitor floor, unlimited,
bounded, and the mutator's four limit combinations. Behavioral tests verify
that unlimited and bounded readers produce the same callback sequence.

## Downstream safety boundary

The three unlimited decoder uses in `rs-redact` have distinct responsibilities:
the independent Serde publication path uses policy input/value limits and an
explicit stack for domain-scope admission; the obsolete two-stage HTTP NDJSON
function has been removed; the enabled JSON-text path reuses the admitted
`Value`, while the disabled path performs only a bounded copy and does not
pretend to parse. These paths no longer contain `JsonDecoder::unlimited()`,
and each non-empty NDJSON line is parsed once.

## Verification requirements

Every observable behavior needs a regression test. Final verification includes
`align-ci.sh`, `ci-check.sh`, documentation tests, Miri, fuzzing, coverage,
feature matrices, downstream compilation, and fixed-CPU Criterion encode and
tree benchmarks. Benchmark records must state the machine and input shape.
