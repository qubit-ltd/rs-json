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
| `jcs_bench` | JCS encoding of large objects, deep objects, and large arrays, with bounded and unlimited configurations and a direct canonicalizer control; `jcs-depth` measures single-child arrays at increasing depths |
| `decoder_bench` / `jcs_decode` | JCS text decoding of number arrays, nested objects, ordinary/escaped keys, and duplicate-key rejection with finite/unlimited budgets and fresh/reused sessions |

Run from the repository root:

```bash
cargo bench --bench decoder_bench
cargo bench --bench budgeted_serde_json -- --noplot
cargo bench --bench tree_bench -- --noplot
cargo bench --features jcs --bench jcs_bench -- --noplot
cargo bench --features jcs --bench decoder_bench -- jcs_decode --noplot
JCS_ALLOCATION_DIAGNOSTICS=1 cargo bench --features "jcs jcs-allocation-diagnostics" --bench jcs_bench -- --noplot
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

The `jcs_decode` group in `decoder_bench` covers 1 KiB and 64 KiB numeric
arrays, a 64-container object, plain and escaped key spellings, and duplicate
key rejection. Successful fixtures are asserted before timing. Unlimited and
finite configurations each measure fresh decoder construction; reused-session
cases run a bounded batch of eight successful decodes in one decoder. Every
batch starts with a new session, so usage can accumulate within that batch but
cannot exhaust one session across Criterion samples.

The JCS target exercises three materially different `Value` shapes: objects
with many members (sorting-table cost), deeply nested objects (traversal
stack cost), and large arrays (many values with little sorting state). It
compares finite value/output/working-memory limits with `JcsEncoder::unlimited`
and the previous `serde_json_canonicalizer` path on the same fixture. The
canonicalizer control measures its own end-to-end output path; it does not
provide the new working-memory bound. Record fixture dimensions and all three
limits when reporting results.

The first command runs Criterion with only the JCS feature, so its timed
samples use the system allocator without allocation-counting instrumentation.
The second command enables `jcs-allocation-diagnostics` and sets
`JCS_ALLOCATION_DIAGNOSTICS`; it prints one
`JCS_ALLOC` diagnostic for each fixture and each of the finite JCS, unlimited
JCS, and legacy paths, and registers no Criterion timing samples. It counts
successful global allocator calls (`alloc`, `alloc_zeroed`, and `realloc`) and
sums their requested byte sizes for that single operation. These one-shot
diagnostics include encoder state, sorting tables, temporary storage, and
output buffers; they are total requested allocation traffic, not auxiliary
working bytes, peak live memory, or process RSS. The prebuilt input `Value` and
diagnostic formatting are outside the measured operation.

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

For a focused run, preserve Criterion's reported estimate and confidence
intervals along with the machine, Rust version, resolved dependencies, fixture
sizes, limits, and exact command. A single current-code sample without a
same-machine pre-change baseline is descriptive only; it does not establish a
performance improvement or regression. These fixtures do not measure
allocation counts or peak memory.

## Recorded JCS decode sample

One directed sample from the pre-0.12 baseline commit `9d821142ed844f25196b52a609eda45eeacefd35` was collected on 2026-10-22 for
`jcs_decode/numbers_1k/unlimited/fresh/decoder_per_decode`:

- Estimate: 38.014 µs; Criterion interval: 37.633–38.607 µs.
- Throughput: 25.714 MiB/s; interval: 25.320–25.975 MiB/s.
- Criterion used 10 samples after 1 s warm-up and 1 s measurement time;
  it reported one mild high outlier.
- Fixture: generated 1,024-byte array containing 512 integer values.
  Configuration: `JcsDecoder::unlimited()`; a fresh decoder per operation.
- Environment: Intel Core i5-9600K (6 cores, 3.70 GHz), x86_64 Linux
  7.0.0-38-generic, Rust/Cargo 1.94.0, locked dependencies. No CPU affinity
  was set; the host was not isolated and CPU frequency scaling was enabled.
  Gnuplot was unavailable, so Criterion used its Plotters backend.

Command:

```bash
cargo bench --manifest-path /data/working/qubit/rust-common/.worktrees/rs-json/implementation-20261022/Cargo.toml --features jcs --bench decoder_bench -- jcs_decode/numbers_1k/unlimited/fresh --noplot --sample-size 10 --measurement-time 1 --warm-up-time 1
```

This is a single observation from the pre-0.12 baseline commit, not a 0.12 measurement or a before/after comparison. No benchmark was run for the 0.12 candidate in this documentation task. It provides no performance conclusion and does not measure allocation counts
or peak memory. The fixture does not represent escaped strings, large objects,
bounded budgets, reused sessions, or application-level work.
## Recorded JCS depth sample (2026-10-23)

The `jcs-depth` group measures a single-child array around a `null` leaf at
depths 32, 128, 512, 2,048, and 8,192. Each fixture is built iteratively,
checked against expected bytes outside the timed operation, and dismantled
iteratively. It compares `JcsEncoder::unlimited()` with finite limits of
`max_depth = depth + 1`, `max_nodes = depth + 1`,
`max_output_bytes = 2 * depth + 4`, and the existing 1 MiB auxiliary-memory
limit. Output lengths are 68, 260, 1,028, 4,100, and 16,388 bytes.

Environment: base revision `ae26c342c66025302ed27f8e7f6031fdc90994d1`, with
the implementation worktree modified; Rust/Cargo 1.94.0; x86_64 Linux
7.0.0-38-generic; Intel Core i5-9600K, 6 cores at 3.70 GHz. No CPU affinity
was set and CPU frequency scaling was enabled. The host was not isolated.
Criterion used its Plotters backend because Gnuplot was unavailable. The
benchmark profile is optimized with LTO and one codegen unit; timing runs used
the system allocator without the allocation counter. Dependencies were locked.

Each timing batch used 20 samples, 1 second warm-up, and 1 second measurement.
The values below are Criterion's reported time intervals; each numbered batch
was a separate command and baseline. A lack of CPU isolation is visible in the
variation across batches.

| Depth | Path | Batch 1 (µs) | Batch 2 (µs) | Batch 3 (µs) | Successful reallocations | Cumulative realloc request bytes | Output bytes |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 32 | unlimited | 6.0173–7.6997 | 3.5546–4.6375 | 4.1912–5.9725 | 35 | 17,104 | 68 |
| 32 | finite | 7.1184–10.820 | 5.0491–5.8028 | 4.8082–5.6065 | 40 | 31,984 | 68 |
| 128 | unlimited | 8.9541–12.204 | 30.351–41.185 | 25.774–38.384 | 133 | 265,168 | 260 |
| 128 | finite | 27.198–39.262 | 22.775–35.555 | 22.287–31.982 | 140 | 326,128 | 260 |
| 512 | unlimited | 67.779–94.772 | 66.182–78.464 | 65.233–93.017 | 519 | 4,206,544 | 1,028 |
| 512 | finite | 62.206–91.545 | 60.150–93.850 | 99.432–150.00 | 528 | 4,451,824 | 1,028 |
| 2,048 | unlimited | 234.93–318.13 | 257.22–344.40 | 188.67–253.32 | 2,057 | 67,157,968 | 4,100 |
| 2,048 | finite | 412.47–502.65 | 385.67–483.98 | 167.52–268.07 | 2,068 | 68,140,528 | 4,100 |
| 8,192 | unlimited | 962.63–1,724.8 | 865.99–1,183.2 | 480.06–659.14 | 8,203 | 1,073,938,384 | 16,388 |
| 8,192 | finite | 858.12–1,172.3 | 1,764.8–2,698.7 | 1,080.8–1,532.2 | 8,216 | 1,077,870,064 | 16,388 |

Timing commands (with `review-depth-1`, `review-depth-2`, and
`review-depth-3` as the respective `--save-baseline` values):

```bash
cargo bench --bench jcs_bench --features jcs --locked -- jcs-depth --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline review-depth-1
JCS_ALLOCATION_DIAGNOSTICS=1 cargo bench --bench jcs_bench --features "jcs jcs-allocation-diagnostics" --locked -- jcs-depth
```

The allocation command was run separately from all timing batches. Its
diagnostics counted successful `realloc` calls and summed their requested new
sizes; they include allocations throughout the complete encode operation,
including output and admission state. They are not actual bytes copied, peak
live bytes, RSS, or an isolated measurement of the renderer stack.

The count of reallocations grows roughly with depth, while cumulative request
bytes rise much faster: at depth 8,192, the diagnostics reported about 1.07 GB
of requested realloc sizes for a 16,388-byte output. This is evidence of
superlinear allocation-request traffic in this fixture and merits separate
attribution work. It does not establish the number of bytes actually moved or
prove a particular container is responsible. Timing intervals varied across
batches and do not isolate an algorithmic cause. This measurement does not
justify changing production growth policy; no production expansion strategy
was changed in this work.

This is a measurement of the modified candidate only, not a before/after
comparison. The `jcs-depth` benchmarks and diagnostics remain available for
follow-up measurements.

### Renderer capacity attribution sample (2026-10-23)

The renderer-only diagnostic was added behind `jcs-allocation-diagnostics` and
`JCS_RENDERER_ALLOCATION_DIAGNOSTICS`. It reports successful growth count,
the sum of requested capacity bytes, and the maximum actual capacity separately
for the renderer stack and object member tables. These are renderer-owned
capacities; they exclude admission state, output buffers, allocator metadata,
and memory used by the input `Value`. The dedicated benchmark path bypasses the
global allocator counter before its measurement flag is enabled.

Environment: HEAD `2c594e7d4e5c6724e6cc994bfabbc0037c2d38f4` with the shared
review worktree modified; Rust/Cargo 1.94.0; x86_64 Linux
`7.0.0-38-generic`; Intel Core i5-9600K, 6 cores, system `System` allocator.
The host was not isolated, no CPU affinity was set, and CPU frequency scaling
was enabled. Three renderer diagnostic runs produced identical values. Each row
is one successful observation per encoding path in each run; there was no
timing sample or failure sample. Unlimited and finite paths produced identical
renderer capacities for each deep-array fixture.

| Shape | Path | Stack growths | Stack requested bytes | Stack max actual bytes | Object growths | Object requested bytes | Object max actual bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| deep array, depth 32 | both | 32 | 16,896 | 1,024 | 0 | 0 | 0 |
| deep array, depth 128 | both | 128 | 264,192 | 4,096 | 0 | 0 | 0 |
| deep array, depth 512 | both | 512 | 4,202,496 | 16,384 | 0 | 0 | 0 |
| deep array, depth 2,048 | both | 2,048 | 67,141,632 | 65,536 | 0 | 0 | 0 |
| deep array, depth 8,192 | both | 8,192 | 1,073,872,896 | 262,144 | 0 | 0 | 0 |
| large object, 2,048 records | both | 2 | 96 | 64 | 2,049 | 147,456 | 49,152 |
| deep object, depth 128 | both | 129 | 268,320 | 4,128 | 129 | 6,168 | 48 |
| large array, 16,384 items | both | 1 | 32 | 32 | 0 | 0 | 0 |

“Both” denotes `jcs-unlimited` and `jcs-finite-budgets`; the deep-array
diagnostic runs confirmed the values match between those paths. The 8,192-layer
case attributes 1,073,872,896 requested bytes to renderer stack growth, with
262,144 bytes as its largest actual stack capacity. The existing whole-operation
diagnostic in the same worktree produced the same values in three runs:
1,073,938,384 cumulative realloc request bytes for unlimited and 1,077,870,064
for finite budgets at depth 8,192. The renderer-only figures therefore explain
nearly all of the deep-array request traffic; the remaining whole-operation
traffic includes allocations outside the renderer and is not assigned here.
These are repeated diagnostic samples, not a before/after comparison or timing
result, and they do not measure bytes copied, peak live
memory, or RSS. The available measurements do not justify changing production
growth policy.

Commands:

```bash
JCS_ALLOCATION_DIAGNOSTICS=1 cargo bench --bench jcs_bench --features "jcs jcs-allocation-diagnostics" --locked -- jcs-depth
JCS_RENDERER_ALLOCATION_DIAGNOSTICS=1 cargo bench --bench jcs_bench --features "jcs jcs-allocation-diagnostics" --locked -- jcs-depth
```

The `jcs-depth` filter also runs the benchmark's explicit large-object control
and reports its stack/object split. The renderer-specific run emits no
`JCS_ALLOC` reports, so diagnostic formatting cannot contaminate global
allocator counts.

### Unlimited renderer stack growth candidate (2026-10-23)

After the renderer attribution sample above, a local candidate changed only
unlimited JCS renderer-stack reservations to request geometrically growing
capacity. Object member tables and finite working-memory configurations keep
the exact-growth policy. The candidate was measured against source revision
`8885285bcfc601e2fe85350f868c05e689441325`; the only production-code
difference in the candidate was `src/encode/internal/jcs_working_memory.rs`.

Environment: Rust/Cargo 1.94.0; x86_64 Linux `7.0.0-38-generic`; Intel Core
i5-9600K, 6 cores at 3.70 GHz; system `System` allocator. The host was not
isolated, CPU affinity was not set, and frequency scaling was enabled. Gnuplot
was unavailable, so Criterion used Plotters. Each batch used 20 samples, a
1-second warm-up, and a 1-second measurement. Intervals are Criterion's
reported estimates; an additional baseline depth batch was interleaved after
the first candidate depth batch.

| Depth | Path | Before batches 1 / 2 / 3 (µs) | After batches 1 / 2 / 3 (µs) |
| ---: | --- | --- | --- |
| 32 | unlimited | 1.887–1.916 / 1.871–1.908 / 1.844–1.883 | 1.075–1.079 / 1.080–1.099 / 1.060–1.082 |
| 128 | unlimited | 8.751–10.242 / 7.638–7.776 / 7.705–7.803 | 3.326–3.353 / 3.526–3.703 / 3.187–3.299 |
| 512 | unlimited | 28.212–29.824 / 27.046–27.425 / 27.342–27.832 | 10.801–11.131 / 11.296–11.802 / 10.396–10.570 |
| 2,048 | unlimited | 112.23–115.39 / 110.62–112.83 / 111.04–112.70 | 40.178–41.432 / 38.503–39.063 / 38.616–39.368 |
| 8,192 | unlimited | 432.99–442.81 / 422.73–432.72 / 428.36–440.33 | 150.06–151.78 / 148.33–150.06 / 147.73–150.27 |
| 8,192 | finite | 632.17–651.57 / 618.23–634.65 / 604.40–616.33 | 616.23–636.01 / 601.60–617.14 / 594.13–607.47 |

Renderer diagnostics at depth 8,192 changed from 8,192 stack growths and
1,073,872,896 requested bytes to 13 growths and 524,224 requested bytes. The
largest actual stack capacity remained 262,144 bytes. Object-table behavior
was unchanged. The finite path retained 8,192 stack growths and the original
request policy. This counts requested capacity bytes, not bytes copied, peak
live memory, or RSS.

Large-object unlimited intervals were 0.951–0.986, 0.947–0.971, and
0.969–0.990 ms before, and 0.944–0.964, 0.943–0.969, and 1.432–1.598 ms
after. The last after batch was an isolated slow sample on this non-isolated
host. Large-array unlimited intervals were 1.327–1.354, 1.358–1.382, and
1.445–1.827 ms before, and 1.312–1.330, 1.318–1.347, and 1.317–1.338 ms
after. These samples show no repeatable wide-container regression, though the
noisy large-object result limits the strength of that conclusion.

The candidate is retained: the deep-chain improvement is repeatable, the
finite path and object-table strategy are unchanged, and wide-container
measurements do not show a repeatable regression. This local measurement does
not establish performance on other machines or workloads.

### Finite renderer stack growth investigation (2026-10-23)

The candidate used geometric stack growth under finite working-memory limits,
required-capacity fallback, and speculative stack-capacity recovery before an
object table reservation. The unlimited path retained its prior direct
`try_reserve_exact` strategy. The candidate was reverted after it failed the
wide-control performance screen; RSJSON-003 remains incomplete. The results
below preserve the investigation evidence and do not describe the current
source tree as optimized.

Environment: Rust/Cargo 1.94.0; x86_64 Linux `7.0.0-38-generic`; Intel Core
i5-9600K, 6 cores at 3.70 GHz; system `System` allocator. The host was not
isolated, CPU affinity was not set, and frequency scaling was enabled. Gnuplot
was unavailable, so Criterion used Plotters. Timing batches used 20 samples,
1-second warm-up, and 1-second measurement, with sequential commands. Intervals
are Criterion slope intervals; point estimate comes first. Depth times are µs;
wide times are ms.

Baseline was clean HEAD `fb1045b0fa98009883ace85a84738d1a5811cc0b`. Candidate
measurements used the same HEAD with uncommitted T3 changes. The JCS benchmark
fixtures and parameters were unchanged. All before and candidate timing
commands exited 0. Three final depth timing commands and six final wide timing
commands (object/array, each covering finite, unlimited, and legacy paths)
exited 0. Three final diagnostic commands with
`JCS_RENDERER_ALLOCATION_DIAGNOSTICS=1` and features `jcs jcs-allocation-diagnostics`
exited 0 and emitted nonzero samples for all registered paths. Timing and
diagnostics were separate.

Deep-array fixtures used `FINITE_WORKING_BYTES = 1 MiB`,
`max_depth = max_nodes = depth + 1`, and `max_output_bytes` equal to complete
canonical output length (depths 512, 2,048, and 8,192). The large-object fixture
had 111,531 output bytes and 6,145 value nodes; the large-array fixture had
87,195 output bytes and 16,385 nodes. Wide finite controls used the same 1 MiB
working-memory limit and exact output/node limits.

| Depth path | Before batches 1 / 2 / 3 (point [interval]) | Candidate batches 1 / 2 / 3 (point [interval]) |
| --- | --- | --- |
| 512 finite | 37.435 [37.078–37.900] / 37.320 [37.039–37.657] / 37.814 [37.226–38.568] | 22.16 [22.02–22.32] / 22.75 [22.47–23.10] / 22.36 [22.13–22.61] |
| 2,048 finite | 145.780 [144.810–147.217] / 147.254 [145.798–148.967] / 148.634 [147.123–150.941] | 82.41 [81.97–82.89] / 81.76 [81.30–82.41] / 82.26 [81.63–83.17] |
| 8,192 finite | 613.655 [608.435–619.985] / 621.189 [607.887–634.761] / 636.312 [621.982–652.576] | 379.10 [374.00–384.82] / 377.00 [374.14–380.55] / 371.85 [368.95–375.66] |
| 512 unlimited | 10.584 [10.474–10.674] / 10.482 [10.419–10.557] / 10.478 [10.386–10.559] | 12.34 [12.31–12.38] / 12.09 [11.99–12.26] / 12.28 [12.18–12.40] |
| 2,048 unlimited | 40.112 [39.672–40.687] / 38.753 [38.520–39.000] / 39.321 [38.878–39.767] | 43.53 [43.24–43.87] / 45.37 [44.95–45.93] / 44.70 [44.30–45.15] |
| 8,192 unlimited | 149.927 [148.941–151.029] / 150.973 [149.258–152.398] / 149.528 [148.523–150.611] | 171.52 [168.21–176.73] / 172.80 [170.17–175.27] / 171.83 [169.92–174.20] |

Finite deep paths improved by more than 10% in all three batches at each depth.
Unlimited depth 8,192 regressed by about 14–15% in all three batches; depth
2,048 also regressed by about 8–16%, and depth 512 by about 15–18%. The finite
8,192 diagnostic reported 13 growths, 524,224 requested bytes, and 262,144
maximum actual bytes in each run, versus 8,192 growths, 1,073,872,896 requested
bytes, and 262,144 maximum actual bytes before the candidate. Candidate
large-object and large-array diagnostics reported no stack recovery. During
candidate verification, a focused tracker regression passed for recovery of
speculative `u64` stack capacity from four elements to three (24 requested
bytes), allowing a two-entry table to fit a 40-byte budget while preserving
stack values `[0, 1, 2]`. The regression was removed with the reverted
implementation.

| Wide control | Path | Before batches 1 / 2 / 3 (point [interval]) | Candidate batches 1 / 2 / 3 (point [interval]) |
| --- | --- | --- | --- |
| Large object | finite | 1.160 [1.142–1.182] / 1.127 [1.117–1.137] / 1.227 [1.163–1.317] | 1.884 [1.730–2.086] / 1.191 [1.171–1.214] / 1.233 [1.210–1.253] |
| Large object | unlimited | 0.939 [0.933–0.948] / 0.947 [0.939–0.957] / 0.948 [0.936–0.961] | 1.506 [1.415–1.597] / 1.015 [1.003–1.025] / 1.045 [1.027–1.072] |
| Large object | legacy | 2.741 [2.678–2.800] / 2.674 [2.636–2.712] / 2.738 [2.676–2.831] | 3.072 [3.047–3.098] / 2.940 [2.890–2.975] / 2.784 [2.720–2.846] |
| Large array | finite | 1.801 [1.781–1.817] / 1.795 [1.774–1.814] / 1.800 [1.786–1.819] | 1.872 [1.857–1.886] / 1.869 [1.862–1.877] / 1.850 [1.848–1.853] |
| Large array | unlimited | 1.350 [1.337–1.365] / 1.336 [1.323–1.352] / 1.354 [1.343–1.369] | 1.528 [1.464–1.621] / 1.429 [1.418–1.440] / 1.446 [1.429–1.474] |
| Large array | legacy | 1.402 [1.381–1.429] / 1.407 [1.397–1.415] / 1.397 [1.387–1.407] | 1.425 [1.415–1.434] / 1.433 [1.430–1.436] / 2.091 [1.518–2.567] |

The unlimited large-object path regressed by more than 5% in two candidate
batches; unlimited large-array regressed by more than 5% in all three. The
finite large-array path stayed below 5%; finite large-object had one candidate
batch above 5%, with the first candidate batch visibly noisy. The final legacy
large-array batch was noisy. Because unlimited wide paths failed the repeated
regression threshold, the candidate was reverted despite the finite deep-chain
improvement. These measurements are an engineering screen on a shared,
frequency-scaled host and do not establish statistical significance.

Diagnostics count requested capacity and maximum actual vector capacity. They
do not measure bytes copied, RSS, allocator metadata, or temporary coexistence
of old and replacement vectors. The measurements describe this candidate and
these fixtures only.

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


### Finite JCS stack-growth candidate trials (2026-10-24)

RJ-004 was **not achieved**. Two finite-stack candidates were measured and withdrawn; no candidate production code or candidate tests remain in the worktree. The first trial accidentally applied finite-budget calculations to unlimited reservations. It was retained as a withdrawn trial and not used to claim success. A corrected trial restored the prior unlimited reservation selection and avoided finite-budget calculations for unlimited object reservations. Finite deep chains improved, but the corrected candidate still exceeded the allowed regression on unlimited deep-chain controls in at least two of three batches at depths 512, 2,048, and 8,192. Per the acceptance rule, the candidate was withdrawn. The controlled recheck confirmed that the poor third finite-8,192 batch was noise; it did not remove the repeated unlimited-control regressions.

**Environment and identity.** HEAD was `bad907715b54728bb4a7bbd221d0d22fb3b27527`; the benchmark worktree also contained uncommitted RJ-001–003 changes. The before source was the pre-T4 `jcs_working_memory.rs` and `jcs_renderer.rs`; the withdrawn and corrected trials were uncommitted T4 candidates over the same HEAD and RJ-001–003 worktree. Rust `1.94.0 (4a4ef493e 2026-03-02)`, Cargo `1.94.0 (85eff7c80 2026-01-15)`, x86_64 Linux `7.0.0-38-generic`, Intel Core i5-9600K (6 cores, 3.70 GHz), system `System` allocator. `Cargo.lock` SHA-256: `05f3bf96eb21153f837a5e590c5e3270f0c71e07cbdcd75118c45fab128386db`. Gnuplot was unavailable; Criterion used Plotters. The host was not isolated, CPU affinity was not set, and CPU frequency scaling was enabled. Timing used the system allocator, no diagnostics, 20 samples, 1-second warm-up, and 1-second measurement. Commands were serialized.

`jcs-depth` used iteratively constructed single-child array chains at depths 32, 128, 512, 2,048, and 8,192. Finite limits were 1 MiB working memory, `max_depth = max_nodes = depth + 1`, and `max_output_bytes` equal to output length (68, 260, 1,028, 4,100, and 16,388 bytes). `jcs-encode` used the existing large-object (111,531-byte output), deep-object (2,719-byte output), and large-array (87,195-byte output) fixtures; finite runs used 1 MiB working memory, exact output-byte limits, and exact node limits. Legacy runs used `serde_json_canonicalizer`.

The tables show Criterion `mean.point_estimate` and its 95% confidence interval from each `estimates.json`, converted to microseconds. Rows retain each benchmark path and fixture identifier. `Withdrawn` data are preserved separately from the corrected candidate.

**Depth benchmarks (µs).**

| Case | Before 1 | Before 2 | Before 3 | Withdrawn 1 | Withdrawn 2 | Withdrawn 3 | Corrected 1 | Corrected 2 | Corrected 3 | Controlled check |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `jcs-depth/finite/128` | 10.465 [10.409–10.523] | 10.732 [10.633–10.846] | 13.132 [11.835–14.671] | 7.941 [7.555–8.375] | 6.360 [6.310–6.411] | 6.414 [6.361–6.475] | 6.242 [6.187–6.314] | 6.120 [6.059–6.187] | 6.301 [6.195–6.426] | 5.964 [5.915–6.028] |
| `jcs-depth/finite/2048` | 147.591 [147.297–147.977] | 146.156 [145.055–147.727] | 149.304 [146.801–152.140] | 78.370 [77.823–78.946] | 81.303 [80.436–82.264] | 79.137 [78.408–80.326] | 76.404 [76.302–76.520] | 76.806 [76.351–77.315] | 77.579 [77.098–78.145] | 76.727 [75.938–77.679] |
| `jcs-depth/finite/32` | 3.178 [3.159–3.199] | 3.209 [3.187–3.231] | 3.167 [3.149–3.184] | 2.163 [2.157–2.171] | 2.273 [2.249–2.298] | 2.205 [2.188–2.224] | 2.128 [2.107–2.156] | 2.143 [2.131–2.159] | 2.140 [2.135–2.145] | 2.051 [2.034–2.069] |
| `jcs-depth/finite/512` | 40.980 [39.269–43.189] | 37.642 [37.336–38.091] | 38.145 [37.892–38.413] | 29.372 [25.568–34.332] | 27.582 [26.044–29.549] | 32.610 [28.990–36.679] | 21.054 [20.766–21.403] | 21.038 [20.926–21.162] | 25.684 [24.415–27.147] | 20.653 [20.456–20.882] |
| `jcs-depth/finite/8192` | 637.208 [628.745–646.800] | 1066.020 [941.025–1218.300] | 1024.688 [911.719–1152.247] | 351.437 [347.127–355.976] | 579.211 [532.775–626.708] | 357.839 [350.376–366.024] | 388.022 [366.470–417.688] | 360.351 [357.474–363.485] | 1110.604 [942.546–1279.832] | 346.325 [341.103–352.269] |
| `jcs-depth/unlimited/128` | 3.147 [3.137–3.158] | 3.149 [3.125–3.177] | 3.211 [3.195–3.229] | 3.513 [3.496–3.529] | 3.526 [3.500–3.556] | 3.509 [3.505–3.514] | 3.251 [3.220–3.286] | 3.299 [3.272–3.330] | 3.284 [3.257–3.320] | 3.267 [3.230–3.310] |
| `jcs-depth/unlimited/2048` | 37.519 [37.456–37.596] | 37.670 [37.547–37.805] | 43.366 [39.457–48.989] | 51.026 [48.954–53.475] | 45.384 [44.880–45.941] | 52.871 [49.993–56.729] | 40.340 [40.033–40.685] | 40.494 [40.220–40.782] | 44.012 [41.881–47.939] | 40.749 [40.377–41.166] |
| `jcs-depth/unlimited/32` | 1.054 [1.047–1.062] | 1.064 [1.052–1.077] | 1.054 [1.048–1.060] | 1.179 [1.169–1.196] | 1.280 [1.211–1.369] | 1.165 [1.163–1.166] | 1.100 [1.088–1.112] | 1.099 [1.092–1.106] | 1.113 [1.109–1.118] | 1.090 [1.083–1.097] |
| `jcs-depth/unlimited/512` | 10.071 [10.030–10.116] | 10.080 [9.973–10.201] | 14.529 [12.976–16.239] | 15.546 [14.155–17.441] | 12.236 [12.099–12.391] | 12.351 [12.138–12.618] | 10.923 [10.837–11.011] | 11.067 [11.023–11.118] | 11.057 [10.971–11.159] | 10.823 [10.713–10.945] |
| `jcs-depth/unlimited/8192` | 143.268 [143.023–143.551] | 193.581 [182.094–205.623] | 146.071 [144.287–148.218] | 167.408 [166.299–168.635] | 216.161 [195.699–239.999] | 169.374 [168.508–170.356] | 155.868 [154.895–156.985] | 154.431 [153.953–155.012] | 162.600 [159.134–167.189] | 157.627 [155.045–160.557] |

**Encode benchmarks (µs).**

| Case | Before 1 | Before 2 | Before 3 | Withdrawn 1 | Withdrawn 2 | Withdrawn 3 | Corrected 1 | Corrected 2 | Corrected 3 | Controlled check |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `jcs-encode/deep-object_finite-budgets/2719` | 68.182 [63.891–73.751] | 62.270 [61.834–62.767] | 61.345 [60.873–61.903] | 61.343 [59.596–63.645] | 52.683 [52.303–53.053] | 53.127 [52.559–53.817] | 52.361 [51.871–52.895] | 101.231 [89.651–113.281] | 59.338 [57.996–60.742] | 54.296 [54.018–54.583] |
| `jcs-encode/deep-object_legacy-canonicalizer/2719` | 108.843 [108.636–109.064] | 105.996 [105.318–106.706] | 111.353 [107.732–117.401] | 107.846 [107.145–108.631] | 106.253 [104.932–107.834] | 107.831 [105.911–110.019] | 106.903 [105.952–107.972] | 183.246 [167.906–201.311] | 108.327 [107.369–109.411] | 106.895 [106.105–107.831] |
| `jcs-encode/deep-object_unlimited/2719` | 45.591 [45.352–45.843] | 44.579 [44.050–45.205] | 44.833 [44.153–45.651] | 44.524 [44.218–44.837] | 43.593 [42.970–44.371] | 42.585 [42.242–42.997] | 44.038 [43.458–44.635] | 102.945 [93.192–112.726] | 55.695 [51.481–60.645] | 43.579 [43.193–44.046] |
| `jcs-encode/large-array_finite-budgets/87195` | 1808.916 [1802.071–1816.677] | 1949.768 [1836.689–2119.260] | 3112.821 [2787.378–3436.353] | 2072.305 [1983.395–2185.788] | 1770.962 [1767.127–1775.422] | 1783.472 [1764.924–1807.330] | 1812.147 [1789.620–1838.364] | 1886.052 [1838.078–1943.951] | 1802.797 [1800.132–1805.698] | 1814.216 [1804.122–1825.881] |
| `jcs-encode/large-array_legacy-canonicalizer/87195` | 1447.362 [1440.191–1456.147] | 1434.690 [1426.934–1443.005] | 1717.390 [1565.254–1896.989] | 1449.960 [1437.056–1464.345] | 1428.058 [1427.265–1428.966] | 1420.139 [1410.594–1430.238] | 1426.863 [1413.502–1440.651] | 1621.535 [1525.243–1733.956] | 1441.961 [1426.680–1463.460] | 1445.313 [1429.512–1464.289] |
| `jcs-encode/large-array_unlimited/87195` | 1360.194 [1358.966–1361.517] | 1857.784 [1662.430–2085.076] | 2280.332 [2043.726–2514.082] | 1377.189 [1369.077–1386.547] | 1368.050 [1367.076–1369.141] | 1371.590 [1367.353–1377.501] | 1380.994 [1368.084–1397.332] | 2162.397 [1995.076–2325.826] | 1381.881 [1377.355–1387.387] | 1397.757 [1387.317–1409.426] |
| `jcs-encode/large-object_finite-budgets/111531` | 1218.696 [1199.025–1241.913] | 1426.773 [1345.397–1513.006] | 1221.439 [1192.712–1252.877] | 1181.661 [1138.235–1235.343] | 1238.204 [1206.479–1271.328] | 1134.699 [1121.900–1149.169] | 1185.254 [1175.572–1196.429] | 2017.663 [1826.223–2237.987] | 1201.736 [1184.153–1222.605] | 1166.433 [1151.690–1183.929] |
| `jcs-encode/large-object_legacy-canonicalizer/111531` | 2929.866 [2910.860–2951.770] | 2832.857 [2793.566–2876.740] | 3100.406 [2989.372–3238.749] | 2794.435 [2739.033–2856.662] | 2953.444 [2842.517–3074.439] | 2735.172 [2694.844–2780.741] | 2718.487 [2668.042–2780.357] | 3542.969 [3380.194–3761.370] | 6245.949 [5283.485–7231.849] | 2866.311 [2814.910–2915.533] |
| `jcs-encode/large-object_unlimited/111531` | 1010.208 [990.096–1033.338] | 995.377 [973.560–1026.985] | 1886.477 [1647.497–2154.265] | 961.573 [954.567–968.580] | 1059.460 [1023.060–1098.025] | 965.909 [951.078–983.060] | 1012.243 [1000.848–1024.296] | 1753.644 [1614.197–1891.854] | 1062.229 [1034.007–1102.836] | 989.711 [979.131–1001.175] |

The corrected finite 512 and 2,048 deep-chain point estimates improved by more than 10% in all three batches. At finite depth 8,192 the first two corrected batches improved 39.1% and 66.2%; batch 3 measured 1,111 µs versus 1,025 µs before (+8.4%) with a broad 95% interval of 943–1,280 µs. The one controlled recheck measured 346.325 µs [341.103–352.269] versus before batch 1 at 637.208 µs, confirming the third batch was noisy.

The corrected unlimited deep-chain control exceeded the 5% regression allowance in at least two of three batches at each target depth: depth 512 (+8.5%, +9.8%, −23.9%), 2,048 (+7.5%, +7.5%, +1.5%), and 8,192 (+8.8%, −20.2%, +11.3%). The single controlled recheck remained +7.5%, +8.6%, and +10.0% versus the corresponding first before batches at those depths. Wide and legacy encode controls also contained large batch-to-batch excursions on the unisolated host; the controlled recheck returned those values near baseline but does not erase the original measurements. As the unlimited deep-chain gate failed, the final status is **RJ-004 not achieved** and no performance improvement is claimed for the released worktree.

**Separate allocation diagnostic.** The command below was run independently from timing (exit 0; it was repeated once to capture complete output, also exit 0). At depth 8,192, finite and unlimited each reported 13 stack growths, 524,224 requested stack-capacity bytes, and 262,144 maximum actual stack-capacity bytes. Stack reclamation count was zero for the deep-array fixture because it has no objects. Other diagnostic samples: large object—2,049 object-table growths, 147,456 requested bytes, 49,152 maximum actual bytes; deep object—8 stack growths / 16,320 requested / 8,192 maximum actual, and 129 object-table growths / 6,168 requested / 48 maximum actual; large array—1 stack growth / 64 requested / 64 maximum actual. These are requested capacities and maximum vector capacities, not bytes copied, peak live memory, or RSS.

**Correctness checks before measurement.** Both corrected-candidate focused commands exited 0: `cargo test --locked --lib --features jcs jcs_working_memory` (8 passed) and `cargo test --locked --test tests --features jcs encode::jcs_encoder_tests` (24 passed). `git diff --check` on the T4 Rust files exited 0. These checks describe the trial before it was reverted; the worktree now has no T4 source or test diff.

**Commands and exit codes.** All 6 before timing commands, all 6 withdrawn-trial timing commands, all 6 corrected-candidate timing commands, both controlled-recheck commands, and both separate diagnostic invocations exited 0. For each three-batch group, the exact command pair was:

Before: each command pair was run with `--save-baseline` set in turn to `rj004-before-1`, `rj004-before-2`, and `rj004-before-3`:

```bash
cargo bench --locked --features jcs --bench jcs_bench -- jcs-depth --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline BASELINE
cargo bench --locked --features jcs --bench jcs_bench -- jcs-encode --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline BASELINE
```

Withdrawn trial: each command pair was run with `--save-baseline` set in turn to `rj004-after-1`, `rj004-after-2`, and `rj004-after-3`:

```bash
cargo bench --locked --features jcs --bench jcs_bench -- jcs-depth --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline BASELINE
cargo bench --locked --features jcs --bench jcs_bench -- jcs-encode --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline BASELINE
```

Corrected candidate: each command pair was run with `--save-baseline` set in turn to `rj004-after-corrected-1`, `rj004-after-corrected-2`, and `rj004-after-corrected-3`:

```bash
cargo bench --locked --features jcs --bench jcs_bench -- jcs-depth --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline BASELINE
cargo bench --locked --features jcs --bench jcs_bench -- jcs-encode --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline BASELINE
```

Controlled recheck (both commands exited 0; one batch each):

```bash
cargo bench --locked --features jcs --bench jcs_bench -- jcs-depth --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline rj004-after-corrected-recheck
cargo bench --locked --features jcs --bench jcs_bench -- jcs-encode --sample-size 20 --warm-up-time 1 --measurement-time 1 --save-baseline rj004-after-corrected-recheck
```

Diagnostics (each command exited 0; run separately from timing):

```bash
JCS_RENDERER_ALLOCATION_DIAGNOSTICS=1 cargo bench --locked --features 'jcs jcs-allocation-diagnostics' --bench jcs_bench -- jcs-depth
JCS_RENDERER_ALLOCATION_DIAGNOSTICS=1 cargo bench --locked --features 'jcs jcs-allocation-diagnostics' --bench jcs_bench -- jcs-depth
```

Criterion’s original raw outputs and estimates remain under `target/criterion/` using the baseline identifiers shown above. The experiment does not change older benchmark records or earlier failed-candidate evidence.
