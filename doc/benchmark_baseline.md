# `rs-json` JSON performance evidence log

[中文版（完整历史记录）](benchmark_baseline.zh_CN.md)

This document is the English summary and navigation page for the repository's
JSON performance evidence. The Chinese document retains the complete dated
measurements, environment details, A/B experiments, rejected alternatives, and
commit identifiers.

The measurements are same-machine evidence for comparing implementations with
the same input shape. They are not cross-machine performance thresholds, and a
single Criterion run is not sufficient evidence of a regression.

## Covered workloads

- Strict and normalizing decode paths, including plain, Unicode, fenced,
  pretty-printed, and control-character inputs.
- Strict encoding with separate `serde_json`, strict-only, value-only,
  output-only, full-budget, owned/reused-session, incremental-writer, numeric,
  string, object, and `RawValue` comparisons.
- JSON tree reading and mutation for large arrays, large objects, and deeply
  nested values under unlimited and bounded accounting.

## Current evidence-backed decisions

1. The state-aware normalizer scan remains because fixed-CPU A/B measurements
   showed a clear improvement on large inputs without changing normalization
   semantics.
2. An owned-buffer encoder with no output limit writes directly into its
   `Vec<u8>`. It still preserves value accounting and `RawValue` validation.
3. A second strict serializer is not justified: after the unlimited
   owned-buffer optimization, strict-only encoding is already in the same
   range as the direct `serde_json` comparison for the recorded fixture.
4. Unlimited tree traversal skips value accounting while bounded traversal
   retains the original checks. Behavioral tests require both paths to emit
   the same callback sequence.
5. The remaining budgeted-encode cost is concentrated in output and
   materialized-value accounting. No additional optimization is retained
   without fixed-CPU profiler evidence and a stable improvement.

These conclusions preserve the strict number contract, duplicate-key checks,
`RawValue` validation, budget transaction semantics, and incremental-writer
partial-output behavior.

## Reproduction commands

Run benchmarks on an otherwise idle machine and record the CPU, operating
system, Rust toolchain, input shape, commit, and Criterion confidence interval.
The current suites are:

```bash
cargo bench --bench decoder_bench
cargo bench --bench budgeted_serde_json -- --noplot
cargo bench --bench tree_bench -- --noplot
```

For fixed-CPU comparisons on Linux, the historical runs used `taskset -c 3`.
The `--quick` benchmark mode is suitable only for screening a direction; use
Criterion's full sampling before retaining or rejecting an implementation.

## Evidence timeline

| Date | Evidence recorded in the complete log |
| --- | --- |
| 2026-07-23 to 2026-07-24 | Decode baselines and two normalizer scanner A/B experiments |
| 2026-08-14 | Multi-size decode and encode budget-admission baselines |
| 2026-08-18 | Current decoder implementation quick recheck |
| 2026-08-25 to 2026-08-26 | Encoder capability-cache, output-accounting, and remaining-cost experiments |
| 2026-08-31 | Unlimited encoder/tree fast paths and current budgeted-encode hotspot recheck |

See the [complete Chinese evidence log](benchmark_baseline.zh_CN.md) for exact
medians, confidence intervals, machine details, commit identifiers, and the
reason each experimental implementation was kept or rejected.
