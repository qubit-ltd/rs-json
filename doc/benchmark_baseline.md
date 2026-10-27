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

### Structural-size first sample (2026-10-27)

The first structural-size sample was collected at revision `5947f5d` with no
uncommitted code changes. The command was:

```bash
cargo bench --locked --bench unique_decode_bench -- unique_decode_structural --noplot
```

Criterion used its default 100 samples, warm-up, and measurement settings. The
representative finite `unique_typed` / `fresh` cases were:

| Shape | Items | Input bytes | Latency estimate (95% CI) | Allocation count | Requested bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| `wide_object` | 32 | 641 | 11.307 µs (11.232–11.396) | 77 | 6,904 |
| `wide_object` | 512 | 10,241 | 421.56 µs (392.66–448.78) | 1,195 | 108,448 |
| `wide_object` | 4096 | 81,921 | 2.6601 ms (2.6443–2.6773) | 9,557 | 877,016 |
| `record_array` | 32 | 929 | 17.047 µs (16.935–17.175) | 136 | 24,064 |
| `record_array` | 512 | 14,849 | 353.86 µs (337.10–375.93) | 2,060 | 381,184 |
| `record_array` | 4096 | 118,785 | 2.2605 ms (2.2441–2.2792) | 16,399 | 3,047,680 |

At 4096 unique members, the finite fresh duplicate-rejection cases measured
325.81 µs (323.73–328.74) for `early` and 2.3056 ms (2.2629–2.3583) for
`late`. Their input was 81,943 bytes; allocation counts and requested bytes
were 11 / 1,204 and 8,881 / 497,510, respectively.

The host was an Intel Core i5-9600K with Ubuntu 24.04.4 (Linux
7.0.0-38-generic), Rust 1.94.0, Criterion 0.8.2, `serde_json` 1.0.154, and
`qubit-budget` 0.10.0. `Cargo.lock` SHA-256 was
`ae1dfa7427aef55f7c6cd96ea3661d274070d1c6a8de2051e84a4a369cb78a52`. The
system allocator was used; the CPU governor was `powersave`, CPUs 0–5 were
available, and no affinity was set. Other compilation and review work was
active on the host during sampling. Criterion warned that some cases needed
more than its 5-second target to collect 100 samples, then completed those
samples. Treat these results as a descriptive first baseline for this shared
host, not as an idle-machine result or a performance comparison.

### Indexed-path follow-up sample (2026-10-27)

The pre-change path sample was collected at the T1 checkpoint (`7dacc96`) in
the detached baseline worktree. The post-change sample used the indexed key
lookup and path changes in this worktree. Both used Rust 1.94.0, Criterion
0.8.2, the same fixture definitions, 10 samples, and 0.1-second warm-up and
measurement settings. The command for the post-change latency sample was:

```bash
cargo bench --locked --bench unique_decode_bench -- unique_decode_structural --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
```

The table reports the Criterion central estimate for each `wide_object` fresh,
unlimited path. Long-prefix keys use the same counts and values as the matching
pre-change sample. Times are in microseconds except where shown in milliseconds.

| Key style | Items | Path | Before | After |
| --- | ---: | --- | ---: | ---: |
| Short | 32 | `owned_value` | 11.54 µs | 19.27 µs |
| Short | 512 | `owned_value` | 576.93 µs | 1.146 ms |
| Short | 4096 | `owned_value` | 23.533 ms | 13.159 ms |
| Short | 32 | `validated_preserve` | 12.07 µs | 34.47 µs |
| Short | 512 | `validated_preserve` | 553.17 µs | 708.31 µs |
| Short | 4096 | `validated_preserve` | 24.625 ms | 27.072 ms |
| Short | 32 | `value_projection` | 8.10 µs | 14.69 µs |
| Short | 512 | `value_projection` | 628.08 µs | 608.39 µs |
| Short | 4096 | `value_projection` | 25.098 ms | 17.034 ms |
| Long prefix | 32 | `owned_value` | 12.20 µs | 107.24 µs |
| Long prefix | 512 | `owned_value` | 597.74 µs | 739.96 µs |
| Long prefix | 4096 | `owned_value` | 26.273 ms | 13.131 ms |
| Long prefix | 32 | `validated_preserve` | 12.24 µs | 105.42 µs |
| Long prefix | 512 | `validated_preserve` | 587.80 µs | 379.51 µs |
| Long prefix | 4096 | `validated_preserve` | 26.925 ms | 11.158 ms |
| Long prefix | 32 | `value_projection` | 7.13 µs | 58.94 µs |
| Long prefix | 512 | `value_projection` | 508.93 µs | 649.78 µs |
| Long prefix | 4096 | `value_projection` | 26.510 ms | 6.339 ms |

The 10-sample follow-up was noisy: several 4096-item measurements warned that
the requested 0.1-second target was too short, and the 32-item long-prefix
intervals were especially wide. Treat the table as exploratory; it does not
establish a stable speedup or regression. The matching allocation command was
`cargo bench --locked --bench unique_decode_allocations`. For `unique_value`,
the short-key `wide_object` fresh cases reported 184 allocations / 14,120
requested bytes at 32 items, 2,825 / 224,896 at 512, and 22,539 / 1,812,680 at
4096. These are allocation request totals, not live index size. The `BTreeSet`
lookup adds O(total decoded key bytes) auxiliary storage and tree-node overhead;
that storage is not included in the exact JSON value budget. The host remained
the shared Intel Core i5-9600K / Ubuntu 24.04.4 system described above; the
post-change `Cargo.lock` SHA-256 was
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`.

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

### Safe JSON bridge duplicate-key index (2026-10-27)

The `safe_value_bridge` group compares direct `serde_json` deserialization into
`SafeJsonValue` before and after replacing repeated object scans with a
`BTreeSet` membership check. The comparable commands were:

```bash
cargo bench --locked --bench decoder_bench -- safe_value_bridge --save-baseline rj001-before --noplot
cargo bench --locked --bench decoder_bench -- safe_value_bridge --baseline rj001-before --noplot
```

Both runs used Rust 1.94.0, Criterion 0.8.2, default features, and 100 samples
per case on the shared Intel Core i5-9600K / Ubuntu 24.04.4 host described
above. The fixture construction and semantic checks are outside the timed
closure; the timed operation includes decoder creation and dropping its result.
The rows below are representative 4096-member direct-Serde cases. Values are
Criterion central estimates; the original runs also emitted confidence
intervals, but the complete saved Criterion data was removed when the
repository CI script cleaned `target/`. A later rerun overlapped unrelated
compilation on the shared host and showed substantial outliers, so it is not
used to replace these observations.

| Key style | Case | Items | Input bytes | Before (95% CI) | After estimate |
| --- | --- | ---: | ---: | ---: | ---: |
| Short | Unique | 4096 | 52,139 | 85.253 ms (78.471–92.320) | 12.838 ms |
| Short | Duplicate at tail | 4096 | 52,152 | 28.743 ms (27.711–29.995) | 2.951 ms |
| Long shared prefix | Unique | 4096 | 121,771 | 48.878 ms (44.369–53.792) | 6.534 ms |
| Long shared prefix | Duplicate at tail | 4096 | 121,801 | 35.167 ms (32.867–37.696) | 4.433 ms |

The first post-change run also showed a small-object cost: at 32 members, direct
Serde unique cases were roughly 77–82% slower than the recorded pre-change
sample. The later noisy rerun varied widely, including apparent improvements
and regressions in 4096-item control paths. These measurements support the
wide-object trend for this host, not a universal latency guarantee. The
`JsonDecoder` cases are controls and do not exercise the changed bridge path.
The index stores each decoded key once and compares ordered strings; lookup
cost depends on shared prefixes, and auxiliary memory grows with total key
bytes plus tree-node overhead.

### Token formatter output-limit action storage (2026-10-27)

The `token_format_limit` group measures successful unlimited formatting and
`OutputLimit` at an output limit of 8 bytes for arrays of 32, 512, and 4096
items (65, 1,025, and 8,193 input bytes). Before/after commands:

```bash
cargo bench --locked --bench budgeted_serde_json -- token_format_limit --save-baseline rj002-before --noplot
cargo bench --locked --bench budgeted_serde_json -- token_format_limit --baseline rj002-before --noplot
```

Criterion used 100 samples per case on the same shared host, Rust 1.94.0,
Criterion 0.8.2, and default features. The representative central estimates
reported by Criterion were:

| Items | Input bytes | Unlimited before → after mean (µs; 95% CI; change, 95% CI) | Limit 8 before → after mean (µs; 95% CI; change, 95% CI) |
| ---: | ---: | ---: | ---: |
| 32 | 65 | 2.6466 (2.6431–2.6504) → 6.1948 µs (5.7068–6.6971; +134.07%, +116.23% to +154.33%) | 2.4360 (2.4094–2.4660) → 4.2857 µs (3.9061–4.6961; +75.93%, +60.22% to +92.13%) |
| 512 | 1,025 | 37.3556 (36.7373–38.0618) → 50.8842 µs (47.6950–54.2535; +36.22%, +27.06% to +45.00%) | 44.8948 (41.9409–48.0430) → 27.3277 µs (27.0175–27.6681; −39.13%, −43.20% to −34.75%) |
| 4096 | 8,193 | 523.0789 (484.2250–563.7111) → 294.0991 µs (290.3670–298.2947; −43.78%, −47.92% to −39.19%) | 441.2246 (413.5106–470.0707) → 358.1130 µs (330.5324–387.6673; −18.84%, −26.23% to −9.28%) |

The change trades a measured small-input regression for lower measured cost on
wide inputs. The private capacity regression verifies that `Some(0)` retains no
actions and that retained action length/capacity do not grow with tokens after
the output limit is exceeded. This bounds the planner's saved action storage;
it is not an RSS or total-memory bound. The complete input is still scanned to
validate syntax and preserve later syntax-error precedence, so scan time and
input storage remain proportional to input size.

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
