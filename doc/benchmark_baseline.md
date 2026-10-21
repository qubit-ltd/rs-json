# JSON benchmark guide

[简体中文](benchmark_baseline.zh_CN.md)

This guide describes the benchmark targets in this checkout and how to compare
changes. Results depend on the hardware, inputs, and enabled limits.

## Benchmark targets

| Target | Coverage |
| --- | --- |
| `decoder_bench` | Strict and normalizing decoding, UTF-8 entry points, downstream-shaped inputs, control characters, and failure paths |
| `budgeted_serde_json` | Decode and encode comparisons with direct `serde_json`, multiple input sizes, and separate budget modes |
| `tree_bench` | Iterative readers and mutators over large arrays, large objects, and deep trees |

Run from the repository root:

```bash
cargo bench --bench decoder_bench
cargo bench --bench budgeted_serde_json -- --noplot
cargo bench --bench tree_bench -- --noplot
```

Use filters for focused measurements:

```bash
cargo bench --bench budgeted_serde_json -- decode --noplot
cargo bench --bench budgeted_serde_json -- encode --noplot
cargo bench --bench budgeted_serde_json -- encode --quick
```

`--quick` is for screening candidates; confirm conclusions with full sampling.
Criterion output is under `target/criterion/` and is generated locally.

## Comparison matrix

The budgeted benchmark uses approximately 1 KiB, 64 KiB, and 1 MiB documents.
Decode cases distinguish direct `serde_json`, owned, borrowed, and reused
sessions, value limits, and normalizing decoding. Encode cases include:

| Mode | Purpose |
| --- | --- |
| `strict-only` | Strict serialization with unlimited budgets |
| `value-only` | Strict serialization with value-resource limits |
| `output-only` | Strict serialization with an output-byte limit |
| `full` | Value and output limits together |
| `strict-owned` / `strict-only` | Fresh and reused unlimited encoders |
| `incremental-writer` / `incremental-output-only` | Incremental output with and without output limits |
| `numeric`, `string`, `raw-value` | Input-shape costs with matching direct `serde_json` controls |

Tree benchmarks distinguish visitor-only traversal, unlimited and bounded
readers, and all four input/output limit combinations for mutators.

## Measurement procedure

1. Record the code revision and uncommitted changes, Rust toolchain, dependency
   resolution, hardware, operating system, input shape, limits, and command.
2. Compare before and after on the same machine with the same fixtures and
   Criterion parameters. Keep CPU affinity and power settings consistent.
3. Confirm successful cases actually succeed. Give each bounded operation a
   fresh session or reset it before reuse so accumulated usage stays comparable.
4. Report latency, throughput, confidence intervals, and the matching direct
   control. Resample the current code rather than treating an older local
   Criterion result as its baseline.
5. Recheck representative bounded and unlimited cases after a focused change.
   Use profiling evidence before introducing a more complex hot-path design.

On Linux with `taskset`, CPU affinity can be fixed to an available CPU, for example:

```bash
taskset -c 3 cargo bench --bench budgeted_serde_json -- --noplot
taskset -c 3 cargo bench --bench tree_bench -- --noplot
```

## Interpretation boundaries

Strict codecs perform checks that direct `serde_json` may not perform. In
particular, the `RawValue` control trusts an already constructed value, while
`JsonEncoder` validates its raw JSON again. The difference measures contract
cost and does not justify removing validation.

Optimizations must preserve numeric ranges, duplicate-key rejection, input and
value accounting, writer error ordering, and partial-output behavior. Numeric
checks should cover `i64`/`u64` boundaries, range rejection, and `NumberBytes`
rejection. Unlimited results apply only to trusted or separately admitted data.
See the [design](json_design.md) and [number contract](number_contract.md).
