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
