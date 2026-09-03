# `rs-json` JSON performance evidence log

[简体中文](benchmark_baseline.zh_CN.md)

> Number-contract note: strict paths accept only the union of the `i64` and
> `u64` integer ranges and finite `f64` values. `serde_json` arbitrary precision
> is not enabled. Numeric rechecks must separately cover the `i64`/`u64`
> boundaries, range rejection, and `NumberBytes` rejection; historical
> arbitrary-precision behavior is not a compatibility baseline.

## Recorded scope

The first record preserves key medians from a complete `decoder_bench` run on
2026-07-23 for downstream-shaped strict byte decoding and normalizing typed
decoding. It supports review of the response and SSE paths in `rs-http`; it is
not a cross-machine or cross-run performance threshold.

Benchmark command:

```bash
cargo +1.94.0 bench --bench decoder_bench
```

The baseline commit was `081fb98f73ba7cd5dc7a930f9d0c98cf70a5bd92`.
The complete run also covered public entry points, control-character
normalization, and representative failures. Criterion's raw statistics remain
under the untracked local `target/criterion/` directory.

## Environment

- CPU: Intel(R) Core(TM) i5-9600K CPU @ 3.70GHz, 6 cores, 1 thread per core.
- Operating system: Linux 6.17.0-35-generic x86_64.
- Toolchain: rustc 1.94.0 (4a4ef493e 2026-03-02), LLVM 21.1.8.

## Strict byte-decode medians

| Input size | `serde_json::from_slice` | Reused strict decoder | Fresh strict decoder |
| --- | ---: | ---: | ---: |
| 1 KiB | 231.56 ns | 239.74 ns | 241.28 ns |
| 64 KiB | 10.978 µs | 10.944 µs | 10.895 µs |
| 1 MiB | 184.94 µs | 185.18 µs | 186.52 µs |

The fresh case constructs `JsonDecoder` inside every Criterion iteration and
then calls `decode_utf8`, matching current `rs-http` response and SSE usage.
All three strict paths are close; comparisons require repeated same-machine
runs and Criterion confidence intervals rather than one measurement.

## Normalizing typed-decode medians

| Input size | Plain JSON | Markdown-fenced JSON | One raw control byte per 1024 bytes |
| --- | ---: | ---: | ---: |
| 1 KiB | 311.00 ns | 355.11 ns | 2.4553 µs |
| 64 KiB | 13.337 µs | 13.656 µs | 241.17 µs |
| 1 MiB | 223.03 µs | 220.23 µs | 2.2784 ms |

These cases use `NormalizingJsonDecoder::with_limits` with the default policy
and unlimited explicit limits, followed by `decode_object_str`. They cover the
plain, fenced, and control-character normalization shapes used by lenient SSE
messages. Control-character density materially changes cost and must be
compared only with historical runs of the same shape.

## 2026-07-24 appended baseline

Commit `76e027c35639542c8df1bac846907cc85e37043d` was measured with a complete
rerun of `cargo +1.94.0 bench --bench decoder_bench`. The environment was
unchanged. Tables use Criterion `median.point_estimate`.

### Strict byte-decode medians

| Input size | `serde_json::from_slice` | Reused strict decoder | Fresh strict decoder |
| --- | ---: | ---: | ---: |
| 1 KiB | 244.69 ns | 242.00 ns | 243.75 ns |
| 64 KiB | 10.959 µs | 11.400 µs | 11.040 µs |
| 1 MiB | 182.17 µs | 188.68 µs | 186.63 µs |

### Normalizing typed-decode medians

| Input size | Plain JSON | Unicode without raw controls | Markdown-fenced JSON | Pretty JSON | One raw control per 1024 bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 KiB | 580.23 ns | 581.39 ns | 615.90 ns | 2.3020 µs | 2.4164 µs |
| 64 KiB | 30.450 µs | 30.397 µs | 29.871 µs | 131.09 µs | 141.79 µs |
| 1 MiB | 506.53 µs | 491.52 µs | 495.86 µs | 2.1129 ms | 2.4197 ms |

The Unicode case isolates normalization pre-scan behavior and is not directly
comparable to control-character input. CPU-frequency state can affect a single
run, so these values are same-machine, same-shape regression evidence only.

## 2026-07-24 control-character pre-scan A/B

A scalar byte-at-a-time pre-scan (A, commit `c422d0b`) was compared with a
pre-scan that safely handles eight bytes at a time (B, commit `82dcbe2`). The
same harness kept verification outside Criterion timing. Runs were pinned to
CPU 3 with Rust 1.94.0, 1 second warm-up, 2 seconds measurement, 50 samples,
and interleaved as `A1-B1-B2-A2-A3-B3`. The table reports the last pair's
medians and Criterion 95% confidence interval for change; the other pairs
confirmed direction. Trial runs with abnormal host load and simultaneous
strict-control drift were excluded.

### Primary 1 MiB paths

| Input shape | A median | B median | B time change relative to A |
| --- | ---: | ---: | ---: |
| Plain JSON | 490.39 µs | 232.54 µs | -53.11% (-53.72% to -52.50%) |
| Unicode without raw controls | 489.64 µs | 233.28 µs | -52.08% (-52.69% to -51.37%) |
| Markdown-fenced JSON | 489.20 µs | 231.39 µs | -51.94% (-52.49% to -51.37%) |
| Pretty JSON | 2.0519 ms | 1.8591 ms | -9.99% (-10.97% to -9.02%) |
| One raw control per 1024 bytes | 2.1805 ms | 2.0044 ms | -7.53% (-8.63% to -6.37%) |

All three no-raw-control 1 MiB paths exceeded the planned 20% improvement
threshold, and the upper confidence bounds still exceeded 10%. Corresponding
1 KiB and 64 KiB paths also improved. Interleaved runs found no reproducible
regression in pretty, sparse-control, dense-control, explicit-normalization-
limit, or strict decode controls. In the third pair, dense-control 1 MiB was
-0.77% (95% CI -1.80% to +0.29%) and reused strict decode was +0.66% (95% CI
-1.83% to +3.89%), neither statistically significant.

The eight-byte pre-scan was retained. Pretty JSON also benefited, but its
remaining 1 MiB cost was still about eight times compact JSON, indicating that
later whitespace compaction dominated and should be evaluated separately.

## 2026-07-24 state-aware ordinary-block skipping A/B

The existing scalar JSON state scan (A, commit
`c87bd2eba740444e6c2e7b605bb30eb1d9c669aa`) was compared with state-aware
eight-byte ordinary-block skipping (B). Both were pinned to CPU 3 and used Rust
1.94.0, 1 second warm-up, 2 seconds measurement, and 50 samples for the full
matrix. Final endpoints were rechecked with 3 seconds and 100 samples.

### Representative endpoints

| Input shape | A median | B median | B time change relative to A |
| --- | ---: | ---: | ---: |
| 1 MiB plain normalizing typed JSON | 231.06 µs | 227.53 µs | -1.53% (-2.48% to -0.19%) |
| 1 MiB pretty normalizing typed JSON | 1.8384 ms | 457.10 µs | -75.14% (-75.60% to -74.83%) |
| 1 MiB, one raw control per 1024 bytes | 1.9980 ms | 546.72 µs | -72.64% (-73.15% to -72.36%) |
| 1 MiB, one raw control per 2 bytes | 12.496 ms | 12.839 ms | +2.75% (+1.51% to +3.42%) |
| 1 KiB, fresh strict decoder | 240.89 ns | 241.12 ns | +0.10% (-0.52% to +0.77%) |

Ordinary-block skipping carries string and backslash state and falls back to
scalar handling for blocks containing quotes, in-string backslashes, or raw
in-string controls. A block with at least two C0 bytes also falls back, avoiding
repeated classification cost for dense malformed strings. The complete matrix
found no reproducible regression above 5%; the previously scalar-bound pretty
and sparse-control 1 MiB paths improved about 75% and 73%, so B was retained.

## 2026-08-14 multi-size `budgeted_serde_json` baseline

This is the comparison baseline before the resource-budget JSON codec rewrite.
Fixtures are compact `{"records":[...]}` arrays containing
`{"id":7,"text":"benchmark-record"}`. Generation verifies that actual size
does not exceed the target or differ by more than one record. Actual sizes are
993 B, 65,533 B, and 1,048,543 B, representing about 1 KiB, 64 KiB, and 1 MiB.
Throughput uses those actual byte counts.

```bash
cargo bench --bench budgeted_serde_json -- --noplot
```

The run was on 2026-08-14 in an uncommitted task tree based on
`97e49b5f575ac0fce0da9a15f8403c1d6fb5456b`. Criterion used its default 3
second warm-up, 5 second measurement, and 100 samples. Latency is
`median.point_estimate`; ratios use same-size `serde_json` as 1.00× and smaller
is faster. This is not a cross-machine threshold.

### Environment

- CPU: Intel(R) Core(TM) i5-9600K CPU @ 3.70GHz, 6 cores, 1 thread per core.
- OS: Linux 6.17.0-35-generic x86_64 GNU/Linux.
- Toolchain: rustc 1.94.0 (4a4ef493e 2026-03-02), LLVM 21.1.8.

### Strict decode

`owned-session` constructs `JsonDecodeSession::from_limits` each iteration;
`borrowed-session` constructs a session borrowing `JsonValueBudget`; and
`reused-session` reuses an unlimited session. The normalizing rows compare the
explicit normalization facade.

| Scenario | ~1 KiB (latency / throughput / ratio) | ~64 KiB (latency / throughput / ratio) | ~1 MiB (latency / throughput / ratio) |
| --- | ---: | ---: | ---: |
| `serde_json` | 3.0582 µs / 309.66 MiB/s / 1.00× | 226.63 µs / 275.77 MiB/s / 1.00× | 3.9649 ms / 252.21 MiB/s / 1.00× |
| `owned-session` | 5.7951 µs / 163.41 MiB/s / 1.89× | 375.03 µs / 166.65 MiB/s / 1.66× | 6.4060 ms / 156.10 MiB/s / 1.62× |
| `borrowed-session` | 5.5218 µs / 171.50 MiB/s / 1.81× | 372.26 µs / 167.89 MiB/s / 1.64× | 6.4005 ms / 156.23 MiB/s / 1.61× |
| `reused-session` | 5.5906 µs / 169.39 MiB/s / 1.83× | 378.06 µs / 165.31 MiB/s / 1.67× | 6.4071 ms / 156.07 MiB/s / 1.62× |
| `NormalizingJsonDecoder::decode_str` | 2.6992 µs / 350.84 MiB/s / 0.88× | 186.22 µs / 335.60 MiB/s / 0.82× | 3.3947 ms / 294.57 MiB/s / 0.86× |
| `NormalizingJsonDecoder::new(...).decode_str` | 2.6619 µs / 355.76 MiB/s / 0.87× | 182.60 µs / 342.25 MiB/s / 0.81× | 3.3478 ms / 298.69 MiB/s / 0.84× |

### Strict encode

Encoding uses the same pre-parsed fixture. The three budgeted cases use a
fresh owned session, a fresh borrowed session, and a reused unlimited session.

| Scenario | ~1 KiB (latency / throughput / ratio) | ~64 KiB (latency / throughput / ratio) | ~1 MiB (latency / throughput / ratio) |
| --- | ---: | ---: | ---: |
| `serde_json` | 1.1311 µs / 837.24 MiB/s / 1.00× | 61.576 µs / 1014.95 MiB/s / 1.00× | 984.02 µs / 1016.20 MiB/s / 1.00× |
| `owned-session` | 5.0364 µs / 188.03 MiB/s / 4.45× | 292.39 µs / 213.74 MiB/s / 4.75× | 4.6477 ms / 215.15 MiB/s / 4.72× |
| `borrowed-session` | 5.0701 µs / 186.78 MiB/s / 4.48× | 291.98 µs / 214.04 MiB/s / 4.74× | 4.6359 ms / 215.70 MiB/s / 4.71× |
| `reused-session` | 5.0570 µs / 187.27 MiB/s / 4.47× | 291.97 µs / 214.05 MiB/s / 4.74× | 4.6374 ms / 215.63 MiB/s / 4.71× |

Budgeted sessions include live structure and payload accounting; encode also
includes transactional output accounting. Absolute time is therefore not
equivalent to direct `serde_json`. Later work must compare identical scenarios
and actual sizes, looking for changes beyond Criterion confidence intervals.

## 2026-08-14 post-admission-rewrite recheck

The same machine, toolchain, and 993 B, 65,533 B, and 1,048,543 B inputs were
used for all 30 scenarios after tasks 1-8 and final clippy/style fixes in a tree
based on `97e49b5f575ac0fce0da9a15f8403c1d6fb5456b`. Criterion settings were
unchanged. Ratios use the new same-size `serde_json`; parenthesized change is
relative to the preceding pre-rewrite data. Runs were not interleaved, so small
shifts can reflect CPU frequency or load.

### Strict decode recheck

| Scenario | ~1 KiB (latency / throughput / ratio / change) | ~64 KiB (latency / throughput / ratio / change) | ~1 MiB (latency / throughput / ratio / change) |
| --- | ---: | ---: | ---: |
| `serde_json` | 2.9135 µs / 325.04 MiB/s / 1.00× / -4.73% | 218.44 µs / 286.10 MiB/s / 1.00× / -3.61% | 3.8917 ms / 256.95 MiB/s / 1.00× / -1.85% |
| `owned-session` | 5.7484 µs / 164.74 MiB/s / 1.97× / -0.81% | 367.50 µs / 170.06 MiB/s / 1.68× / -2.01% | 6.3164 ms / 158.31 MiB/s / 1.62× / -1.40% |
| `borrowed-session` | 5.8399 µs / 162.16 MiB/s / 2.00× / +5.76% | 368.75 µs / 169.48 MiB/s / 1.69× / -0.94% | 6.3221 ms / 158.17 MiB/s / 1.62× / -1.22% |
| `reused-session` | 5.4755 µs / 172.95 MiB/s / 1.88× / -2.06% | 369.54 µs / 169.12 MiB/s / 1.69× / -2.25% | 6.4119 ms / 155.96 MiB/s / 1.65× / +0.07% |
| `NormalizingJsonDecoder::decode_str` | 2.7992 µs / 338.31 MiB/s / 0.96× / +3.70% | 178.91 µs / 349.33 MiB/s / 0.82× / -3.93% | 3.2670 ms / 306.08 MiB/s / 0.84× / -3.76% |
| `NormalizingJsonDecoder::new(...).decode_str` | 5.1220 µs / 184.89 MiB/s / 1.76× / +92.42% | 326.85 µs / 191.21 MiB/s / 1.50× / +79.00% | 5.6367 ms / 177.40 MiB/s / 1.45× / +68.37% |

This historical table predates API cleanup and is not a conclusion about the
current `decode_str()`. Current plain and session-backed `decode_str()` both
perform lexical admission before directly deserializing the target; see the
2026-08-18 quick recheck. Future comparison must resample the same current
implementation rather than interpret this obsolete difference.

Owned, borrowed, and reused general decode paths differed by only about
0.27-0.36 µs at 1 KiB; signs reverse at larger sizes and magnitudes are below
about 2%. Session construction/borrowing is visible only for small inputs and
is not the medium/large optimization priority.

### Strict encode recheck

| Scenario | ~1 KiB (latency / throughput / ratio / change) | ~64 KiB (latency / throughput / ratio / change) | ~1 MiB (latency / throughput / ratio / change) |
| --- | ---: | ---: | ---: |
| `serde_json` | 1.2738 µs / 743.47 MiB/s / 1.00× / +12.61% | 67.110 µs / 931.27 MiB/s / 1.00× / +8.99% | 1.0676 ms / 936.63 MiB/s / 1.00× / +8.50% |
| `owned-session` | 4.8595 µs / 194.88 MiB/s / 3.82× / -3.51% | 288.64 µs / 216.52 MiB/s / 4.30× / -1.28% | 4.6059 ms / 217.11 MiB/s / 4.31× / -0.90% |
| `borrowed-session` | 4.8581 µs / 194.93 MiB/s / 3.81× / -4.18% | 287.83 µs / 217.13 MiB/s / 4.29× / -1.42% | 4.5470 ms / 219.92 MiB/s / 4.26× / -1.92% |
| `reused-session` | 4.8236 µs / 196.32 MiB/s / 3.79× / -4.61% | 288.08 µs / 216.94 MiB/s / 4.29× / -1.33% | 4.5549 ms / 219.54 MiB/s / 4.27× / -1.78% |

The budgeted encode cases remain close: at 1 KiB owned/borrowed are only about
35 ns above reused, with no consistent construction-cost direction at larger
sizes. Budgeted throughput is about 195-220 MiB/s, only 23%-26% of direct
`serde_json`, making it the clearest hotspot in this matrix.

### Priorities supported by this data only

1. Optimize structure, payload, and transactional output accounting in
   budgeted encode, continuously rechecking with the same Criterion command.
2. Evaluate lexical-admission scan cost shared by strict and normalizing decode
   while preserving complete value admission and direct Serde deserialization.
3. Optimize session construction, borrowing, or reuse only after a current
   fixed benchmark shows a stable gain.

## 2026-08-18 current-implementation quick recheck

Quick screening used the following commands. `--quick` determines direction
only and does not replace a full fixed-environment benchmark.

```bash
cargo bench --bench budgeted_serde_json -- --quick
cargo bench --bench budgeted_serde_json -- decode --quick
```

For encode without an output-byte limit, caching whether accounting is needed
avoided borrowing accounting on every serializer `Write` callback. Owned 1 KiB
/ 64 KiB / 1 MiB medians changed from 4.866 / 290.31 / 4412 µs to 4.009 /
232.55 / 3721 µs. Incremental writing changed from 2.761 / 180.92 / 2976 µs
to 2.308 / 155.98 / 2456 µs. Output-limited paths retain every pre-write check
and accepted-byte charge.

On 993 B input, strict and normalizing session paths were about 6.10-6.17 µs,
with no statistically significant change. Both perform lexical admission then
direct target deserialization. Future baselines must resample on the same
hardware with full Criterion settings and report bounded and unbounded cases.

## 2026-08-25 encode capability-cache optimization

The encode benchmark was split into `strict-only`, `output-only`, `value-only`,
and `full`. Before the change, strict mode dynamically borrowed shared context
for every Serde event even without value limits. Integers computed budget-only
lexeme lengths, and sequences/maps maintained budget-only counts. At 1 MiB,
`strict-only` was about 3.40× direct `serde_json`; `full` was about 4.96×. The
primary cost was per-node borrowing, measurement, and container accounting,
not session construction.

The encode entry point now computes `has_value_limits` once and copies that
immutable capability into serializer, compound, nested-value, and key wrappers.
Without value limits they return before borrowing context and skip numeric
lexeme measurement, byte-array element measurement, and container counts.
The 64-bit number union, RawValue syntax scan, and output accounting always
remain. Same-process quick A/B direction was:

| Path | ~1 KiB | ~64 KiB | ~1 MiB |
| --- | ---: | ---: | ---: |
| `strict-only` | 3.5267 → 3.2877 µs (-6.8%) | 208.27 → 173.96 µs (-16.5%) | 3.2188 → 2.8161 ms (-12.5%) |
| `output-only` | 4.4097 → 3.8314 µs (-13.1%) | 255.18 → 217.96 µs (-14.6%) | 3.8645 → 3.5712 ms (-7.6%) |
| `numeric` | 2.5146 → 1.7406 µs (-30.8%) | 140.11 → 87.082 µs (-37.8%) | 2.1975 → 1.3118 ms (-40.3%) |
| `full` | 5.1318 → 4.9785 µs (-3.0%) | 289.23 → 282.29 µs (-2.4%) | 4.6958 → 4.4903 ms (-4.4%) |

Quick results are directional only. The large `numeric` improvement directly
confirmed that unnecessary numeric lexeme measurement was the root cause;
`full` still requires that work and moved near the noise range.

The final baseline used Criterion's default 3 second warm-up, 5 second
measurement, and 100 samples in a working tree based on `40121aa`:

```bash
cargo bench --bench budgeted_serde_json -- encode --noplot
```

Environment: Intel(R) Core(TM) i5-9600K, Linux 6.17.0-35-generic x86_64,
rustc 1.94.0 (LLVM 21.1.8).

| Scenario | ~1 KiB (latency / throughput / serde ratio) | ~64 KiB (latency / throughput / serde ratio) | ~1 MiB (latency / throughput / serde ratio) |
| --- | ---: | ---: | ---: |
| `serde_json` | 1.1748 µs / 806.07 MiB/s / 1.00× | 64.315 µs / 971.74 MiB/s / 1.00× | 1.0118 ms / 988.30 MiB/s / 1.00× |
| `strict-only` | 3.1517 µs / 300.47 MiB/s / 2.68× | 177.50 µs / 352.09 MiB/s / 2.76× | 2.8302 ms / 353.32 MiB/s / 2.80× |
| `output-only` | 3.7749 µs / 250.87 MiB/s / 3.21× | 218.21 µs / 286.41 MiB/s / 3.39× | 3.5173 ms / 284.30 MiB/s / 3.48× |
| `value-only` | 4.1269 µs / 229.47 MiB/s / 3.51× | 235.77 µs / 265.07 MiB/s / 3.67× | 3.7423 ms / 267.21 MiB/s / 3.70× |
| `full` | 4.8979 µs / 193.35 MiB/s / 4.17× | 285.37 µs / 219.00 MiB/s / 4.44× | 4.5259 ms / 220.94 MiB/s / 4.47× |
| `raw-value` | 2.4314 µs / 389.48 MiB/s / 2.07× | 145.49 µs / 429.55 MiB/s / 2.26× | 2.3498 ms / 425.56 MiB/s / 2.32× |

The small optimization was retained. Remaining cost comes from strict Serde
decoration, transaction lifetime, per-event output writing, and enabled value
accounting. Any further output-writer experiment must be isolated from this
capability-cache change.

## 2026-08-25 output-accounting separation experiment

`encode/incremental-output-only` was added so incremental writing used the same
output-byte limit as `encode/output-only`:

```bash
cargo bench --bench budgeted_serde_json -- encode/output-only --quick
cargo bench --bench budgeted_serde_json -- encode/incremental-writer --quick
cargo bench --bench budgeted_serde_json -- encode/incremental-output-only --quick
```

For 993 B / 64 KiB / 1 MiB, buffered `output-only` throughput was about 248 /
287 / 292 MiB/s; unlimited incremental writing was about 566 / 600 / 604 MiB/s;
and equally bounded incremental writing was about 381 / 393 / 377 MiB/s. This
shows measurable output-accounting cost plus independent allocation/copy cost
in the buffered `Vec` path. Evidence did not justify changing accounting
semantics or removing per-write boundary checks, so only the repeatable
comparison benchmark was retained.

## 2026-08-26 remaining encode-cost separation

Before-change quick measurements were pinned to CPU 3 at commit
`0d6db73a2c29838769df44f762f3b6ec913ec91f`. Three same-shape direct
`serde_json` controls were then added: `incremental-serde-json`,
`numeric-serde-json`, and `raw-value-serde-json`. They isolate ordinary
serializer wrapping, numeric-event wrapping, and complete RawValue lexical
validation. Toolchain and CPU remained rustc 1.94.0 / LLVM 21.1.8 and
Intel(R) Core(TM) i5-9600K.

Linux `perf` was unavailable because `kernel.perf_event_paranoid=4`; no usable
flamegraph or valgrind tool was installed. Permissions were not changed.
Shape controls within the same Criterion process were used instead. Final
sampling used 1 second warm-up, 2 seconds measurement, and 50 samples:

```bash
taskset -c 3 cargo bench --bench budgeted_serde_json -- \
  '(65533|1048543|54601|873811)' \
  --warm-up-time 1 --measurement-time 2 --sample-size 50 --noplot
```

Ratios use the direct same-row `serde_json` path as 1.00×. Direct RawValue
trusts the constructed value while `JsonEncoder` revalidates by contract, so
that ratio quantifies contract cost and is not evidence for removing validation.

| Path | ~64 KiB: direct / `JsonEncoder` / ratio | ~1 MiB: direct / `JsonEncoder` / ratio |
| --- | ---: | ---: |
| Ordinary buffered structure | 67.018 / 178.70 µs / 2.67× | 1.0743 / 2.8220 ms / 2.63× |
| Ordinary incremental structure | 22.768 / 103.30 µs / 4.54× | 398.64 µs / 1.6665 ms / 4.18× |
| Numeric array | 63.969 / 82.219 µs / 1.29× | 1.0306 / 1.3013 ms / 1.26× |
| `RawValue` | 1.2525 / 140.00 µs / 111.78× | 31.164 µs / 2.2557 ms / 72.38× |

Ordinary structure primarily pays for re-entering strict serializer/compound
wrappers for each nested value; numeric paths differ by only about 26%-29%;
RawValue is dominated by the required full lexical scan. Without duplicating a
Serde serializer/compound state machine or removing RawValue validation, no
small candidate was likely to meet the 10% primary-endpoint threshold. No
candidate code was retained; the three controls remain for future architecture
work.

This experiment also found a missing contract regression: non-finite `f32` and
`f64` values had been delegated to `serde_json` and emitted as `null`, contrary
to the number contract. The serializer now rejects them in every budget mode.
The regression test failed before the fix and the full text-encode tests passed
after it. Quick primary matrices showed no reproducible regression above 5%.
This is a correctness fix, not a performance claim.

## 2026-08-31 unlimited encode and tree fast paths

First-principles separation identified required work (strict Serde traversal,
number/key validation, byte generation) and removable work (output
buffer/accounting with no output limit; tree measurement/admission with no
value limits). Quick screening was pinned to CPU 3 and used only to select
candidates for full sampling.

### 1 MiB object-array encode quick medians

| Path | Before | After E1 | Decision |
| --- | ---: | ---: | --- |
| `serde_json` | 1.0228 ms | same control | reference |
| `strict-only` | 2.3252 ms | 1.0620 ms | about -54%; retain E1 |
| `value-only` | 3.1161 ms | 1.8543 ms | about -40%; retain E1 |
| `output-only` | 2.9761 ms | 3.2209 ms | E1 not used; verify quick noise with full sampling |
| `full` | 3.7308 ms | 3.9392 ms | E1 not used; verify quick noise with full sampling |

E1 writes directly to the owned `Vec<u8>` when there is no output limit while
retaining value transactions and RawValue failure propagation. Afterwards
`strict-only` was only about 4% from direct `serde_json`, below the theoretical
5% benefit ceiling of duplicating a no-value-budget serializer, so E2 was
rejected. E3 lacked profiler evidence because `kernel.perf_event_paranoid=4`
and permissions were not changed. E4 would couple full RawValue scanning to
output state and cannot emit incremental bytes before validation completes, so
it was rejected. String-heavy, owned/reused-session, and RawValue controls
remain in the suite.

### Representative unlimited tree quick medians

| Scenario | Before | After fast path | Change |
| --- | ---: | ---: | ---: |
| reader array / 1K | 16.2 µs | 8.22 µs | about -49% |
| reader array / 16K | 252 µs | 156.85 µs | about -38% |
| reader object / 256 | 15.5 µs | 10.09 µs | about -35% |
| reader object / 4096 | 269 µs | 164.87 µs | about -39% |
| reader deep tree | 8.66 µs | 5.87 µs | about -32% |
| mutator large array | 669 µs | 157.8 µs | about -76% |
| mutator large object | 1.97 ms | 1.49 ms | about -24% |
| mutator deep tree | 48.5 µs | 30.97 µs | about -36% |

The fast path met the retention threshold; bounded paths kept all original
checks and quick variation was reviewed through full Criterion sampling:

```bash
taskset -c 3 cargo bench --bench budgeted_serde_json -- --noplot
taskset -c 3 cargo bench --bench tree_bench -- --noplot
```

Both completed with Criterion's default 3 second warm-up, 5 second measurement,
and 100 samples. Final 1 MiB encode medians were `serde_json` 1.1117 ms,
`strict-only` 0.9944 ms, `value-only` 1.7384 ms, `output-only` 3.2153 ms, and
`full` 4.0015 ms. The `output-only` change interval crossed zero, so no
regression was detected. `value-only` changed -5.87%, consistent with E1.

Final reader unlimited medians for array 1K/16K, object 256/4096, and deep tree
were 9.93 µs, 157.05 µs, 9.87 µs, 161.64 µs, and 5.74 µs; corresponding
bounded medians were 17.96 µs, 281.65 µs, 16.51 µs, 270.58 µs, and 9.24 µs.
Mutator unlimited medians for large array, large object, and deep tree were
169.53 µs, 1.4316 ms, and 28.51 µs. All four limit combinations completed.
Protected paths varied in both directions from earlier quick samples, but large
differences were insignificant and no stable regression above 3% appeared.
Behavioral tests also confirmed identical reader callback sequences for
unlimited and bounded paths.

## 2026-08-31 current budgeted-encode hotspot recheck

The current tree ran
`cargo bench --bench budgeted_serde_json -- encode --quick`. Same-fixture
`strict-only`, `value-only`, `output-only`, `full`, and incremental controls
separated costs. One MiB medians were:

| Path | Median | Relative to `serde_json` |
| --- | ---: | ---: |
| `serde_json` | 1.1144 ms | 1.00× |
| `strict-only` | 996.43 µs | 0.89× |
| `value-only` | 1.7374 ms | 1.56× |
| `output-only` | 3.1972 ms | 2.87× |
| `full` | 4.2207 ms | 3.79× |
| `incremental-serde-json` | 423.93 µs | 0.38× |
| `incremental-writer` | 1.7001 ms | 1.53× |
| `incremental-output-only` | 2.3780 ms | 2.13× |

The matrix confirms that output accounting and materialized-value accounting
are the main current costs; session construction is not. Criterion changes
against saved baselines did not identify a stable 5% candidate, and output
accounting must preserve per-write boundary checks and partial-output behavior.
No new encode hot-path optimization was committed. Future work requires
fixed-CPU profiler evidence, an isolated A/B of the output writer or admission
plan, and simultaneous verification of budget, I/O, and incremental-output
semantics.
