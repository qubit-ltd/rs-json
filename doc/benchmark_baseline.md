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
| `unique_decode_bench` | Unique-key decoding across config, metadata-map, and wire-envelope fixtures at approximately 1 KiB, 64 KiB, and 1 MiB, with dynamic, typed, typed-seed, ordinary decoder, and direct Serde paths |
| `unique_decode_allocations` | Separate one-shot allocation-request diagnostics for the same paths and duplicate-key rejection; no Criterion samples |

Run from the repository root:

```bash
cargo bench --bench decoder_bench
cargo bench --bench budgeted_serde_json -- --noplot
cargo bench --bench tree_bench -- --noplot
cargo bench --features jcs --bench jcs_bench -- --noplot
cargo bench --features jcs --bench decoder_bench -- jcs_decode --noplot
JCS_ALLOCATION_DIAGNOSTICS=1 cargo bench --features "jcs jcs-allocation-diagnostics" --bench jcs_bench -- --noplot
cargo bench --locked --bench unique_decode_bench -- --noplot
cargo bench --locked --bench unique_decode_allocations
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

The unique-key targets use three owned fixture shapes: a configuration object,
a string metadata map, and a versioned wire envelope with records. Fixture
budgets use actual UTF-8 byte sizes and source node counts. Each shape has successful fixtures
near 1 KiB, 64 KiB, and 1 MiB, plus a separate duplicate-key rejection case.
The Criterion target compares the unique dynamic wrapper, typed decode, typed
seed, ordinary `JsonDecoder`, and direct `serde_json`. Direct controls do not
enforce global duplicate-key rejection. Finite cases cap input and value
resources; reused samples create a fresh decoder per batch and decode eight
documents with cumulative limits sized for the whole batch.

The latency target uses the system allocator. The separate allocation target
prints `UNIQUE_DECODE_ALLOC` records with shape, bytes, path, budget, mode,
allocation count, total requested bytes, and reallocations. Reallocation
requests contribute to total requested bytes and are also counted separately;
they are not newly live bytes. The counter measures successful allocation
request traffic during the operation, not peak live memory, exact auxiliary
working memory, or RSS. Fixture setup, warm-up, result checks, and formatting
are outside the measurement window.

The unique-key benchmarks also include structural-size fixtures in addition to
the byte-size fixtures above. The structural cases cover `wide_object` and
`record_array` with 32, 512, and 4096 members or records. Their scalar strings
remain short as the number of entries grows. Typed paths decode the complete
target (`BTreeMap<String, String>` or `Vec<StructuralRecord>`); dynamic paths
retain the complete `Value`. The direct `serde_json` paths have no finite-budget
variant and do not enforce the decoder's global duplicate-key rejection
contract.

The structural Criterion cases can be selected with:

```bash
cargo bench --locked --bench unique_decode_bench -- unique_decode_structural --noplot
```

They include fresh operations and batches of eight. Fresh timing includes
decoder construction and result destruction. A batch creates its decoder in
Criterion setup, then decodes eight inputs and destroys the results in the
timed operation. Fixture generation, full-result preflight checks, and batch
decoder setup are outside timing. Finite batch preflight uses limits sized for
eight cumulative inputs.

Duplicate rejection cases use a wide object with the same repeated key inserted
early or after all unique members. Both fixtures contain the same number of
unique members and one extra duplicate member; each is checked to fail for
`DuplicateKey` under finite and unlimited budgets.

The allocation target preserves `UNIQUE_DECODE_ALLOC` and adds records with
this schema:

```text
UNIQUE_DECODE_STRUCT_ALLOC,shape,items,bytes,path,budget,mode,allocation_count,allocated_request_bytes,reallocation_count
```

Here `shape` is `wide_object`, `record_array`, `wide_object_reject_early`, or
`wide_object_reject_late`. Batch8 measurements report the total for all eight
decodes. Total requested bytes include successful allocation and reallocation
requests; they describe allocation traffic and do not equal peak live memory
or bytes physically moved.

The JCS target exercises three materially different `Value` shapes: objects
with many members (sorting-table cost), deeply nested objects (traversal
stack cost), and large arrays (many values with little sorting state). It
compares finite value/output/working-memory limits with `JcsEncoder::unlimited`
and a direct `serde_json_canonicalizer` control on the same fixture. The
canonicalizer control measures its own end-to-end output path; it does not
provide the JCS encoder's working-memory bound. Record fixture dimensions and all three
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

### Bounded JCS stack-growth observation (2026-10-26)

The existing single-child-array depth fixture was sampled at revision
`44b70043ef6ded7de68bd6fd6e22581e1b720fef` with Rust 1.94.0, an Intel Core
i5-9600K, Ubuntu 24.04.4 (Linux 7.0.0-38-generic), and Rust's `System`
allocator. The CPU governor was `powersave`; CPUs 0–5 were available and no
CPU affinity was set. The locked run used `serde_json 1.0.154`,
`serde_json_canonicalizer 0.4.1`, `qubit-budget 0.10.0`, and `criterion 0.8.2`;
the `Cargo.lock` SHA-256 was
`ae1dfa7427aef55f7c6cd96ea3661d274070d1c6a8de2051e84a4a369cb78a52`. The
finite fixture used a 1 MiB working-memory limit and exact output, depth, and
node limits. Criterion used its default 100 samples per case. The commands
were:

```bash
cargo bench --locked --features jcs --bench jcs_bench -- jcs-depth --noplot --save-baseline rj002-before
JCS_RENDERER_ALLOCATION_DIAGNOSTICS=1 cargo bench --locked --features 'jcs jcs-allocation-diagnostics' --bench jcs_bench -- --noplot
cargo bench --locked --features jcs --bench jcs_bench -- jcs-encode --noplot --save-baseline rj002-before
```

| Depth | Unlimited estimate (reported confidence interval) | Finite estimate (reported confidence interval) | Stack growths, unlimited / finite | Finite cumulative requested stack bytes |
| ---: | ---: | ---: | ---: | ---: |
| 32 | 1.4781 µs (1.3486–1.6258) | 3.4467 µs (3.3876–3.5111) | 5 / 32 | 16,896 |
| 128 | 3.0330 µs (3.0160–3.0521) | 11.908 µs (11.728–12.163) | 7 / 128 | 264,192 |
| 512 | 11.420 µs (11.176–11.721) | 93.461 µs (80.569–107.09) | 9 / 512 | 4,202,496 |
| 2048 | 140.08 µs (120.90–165.33) | 449.09 µs (398.87–501.24) | 11 / 2048 | 67,141,632 |
| 8192 | 209.08 µs (197.12–221.99) | 848.90 µs (769.90–928.19) | 13 / 8192 | 1,073,872,896 |

For the 8192-depth case, both paths reported a maximum actual stack capacity
of 262,144 bytes. The finite cumulative requested-byte total sums requests
across growth events; it is neither peak/live memory nor bytes actually moved.
The end-to-end finite/unlimited estimates also include different budget
admission work. Representative finite/unlimited estimates were 1.4002/1.0836
ms for the large object, 83.323/48.029 µs for the deep object, and
1.9309/1.4695 ms for the large array. These are descriptive observations,
not a before/after comparison or proof that stack growth alone caused the
latency differences. The exact finite-capacity policy was retained because
speculative stack capacity can consume bytes needed by a later object member
table; reclaiming that capacity would require additional coordination with
renderer admission.

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

Unique-key measurements follow the same boundary: without a matching
pre-change run, report observations for the current revision only. Do not infer
a speedup or regression from different fixtures or machines.

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
