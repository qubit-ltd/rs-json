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
| `lexical_array_allocations` | Requested allocation and retained-result diagnostics for scalar, record, empty, and single-child-chain arrays |

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
cargo bench --locked --bench lexical_array_allocations
cargo bench --locked --bench unique_decode_bench -- lexical_arrays
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
key rejection. Successful fixtures are asserted before timing, including all
eight decodes on one decoder for reused-session preflight; duplicate-key
preflight checks the specific rejection reason. Timed closures decode and
consume each result without checking its success. Unlimited and finite
configurations each measure fresh decoder construction; reused-session cases
run a bounded batch of eight decodes in one decoder. Every batch starts with a
new session, so usage can accumulate within that batch but cannot exhaust one
session across Criterion samples.

The unique-key targets use three owned fixture shapes: a configuration object,
a string metadata map, and a versioned wire envelope with records. Fixture
budgets use actual UTF-8 byte sizes and source node counts. Each shape has successful fixtures
near 1 KiB, 64 KiB, and 1 MiB, plus a separate duplicate-key rejection case.
The Criterion target compares the unique dynamic wrapper, typed decode, typed
seed, ordinary `JsonDecoder`, and direct `serde_json`. Direct controls do not
enforce global duplicate-key rejection. Finite cases cap input and value
resources; reused samples create a fresh decoder per batch and decode eight
documents with cumulative limits sized for the whole batch.

The unique-key latency targets preflight successful single and batch8 results
before registering measurements. Duplicate-rejection results are checked for
`DuplicateKey` before timing, while timed closures only perform and consume the
decode result. This measurement-boundary change does not alter the fixtures,
budgets, decoder lifecycle, result destruction, throughput, or historical
samples. Old and new timing samples use different validation overhead and must
not be compared as evidence of a speedup; no new performance baseline was
collected for this change.

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

### Typed unique-decode peak-live sample (2026-10-29)

This sample checks whether typed unique-key decoding has a measurable
intermediate live-allocation peak before considering an optimization. The
allocation benchmark's opt-in live counter records successful allocator
requests and frees during one decode. `peak_extra` is the high-water mark above
the live bytes at the start of the operation; `end_delta` is the live-byte
difference at the end. These are allocator-request measurements, not RSS or an
exact count of duplicate `Value` materialization. The `protected_value` path is
a control that retains a protected dynamic value; the difference also includes
any other path-specific work and result shape. The implementation worktree
started at `cdcdf2c` and contained only this task's implementation changes
while collecting samples. Fixture setup and the counter self-check were
outside the measured operation. Direct Serde is a separate, non-equivalent
control because it does not enforce global duplicate-key rejection or the
decoder's resource contract.

Each allocation run used one of four feature groups, three times per group:

```bash
cargo bench --locked --bench unique_decode_allocations
cargo bench --locked --features "serde_json/arbitrary_precision" --bench unique_decode_allocations
cargo bench --locked --features "serde_json/preserve_order" --bench unique_decode_allocations
cargo bench --locked --features "serde_json/arbitrary_precision serde_json/preserve_order" --bench unique_decode_allocations
```

The same 4 commands were repeated for runs 2 and 3. Each invocation emitted
390 structural records plus its CSV header. All records had `peak >= baseline`
and `peak >= end`; the three runs were identical. The table reports fresh,
unlimited `unique_typed` versus `protected_value`; numbers are `peak_extra`
bytes, with the control in parentheses.

| Features | Shape | Items | Typed peak extra (control) | Typed minus control |
| --- | --- | ---: | ---: | ---: |
| default | `wide_object` | 512 | 131,616 (90,432) | 41,184 |
| default | `wide_object` | 4096 | 1,065,048 (726,776) | 338,272 |
| arbitrary precision | `wide_object` | 512 | 131,616 (90,432) | 41,184 |
| arbitrary precision | `wide_object` | 4096 | 1,065,048 (726,776) | 338,272 |
| preserve order | `wide_object` | 512 | 200,368 (159,184) | 41,184 |
| preserve order | `wide_object` | 4096 | 1,608,496 (1,270,224) | 338,272 |
| combined | `wide_object` | 512 | 200,368 (159,184) | 41,184 |
| combined | `wide_object` | 4096 | 1,608,496 (1,270,224) | 338,272 |
| default | `record_array` | 512 | 384,512 (364,992) | 19,520 |
| default | `record_array` | 4096 | 3,076,096 (2,916,800) | 159,296 |
| arbitrary precision | `record_array` | 512 | 385,024 (365,504) | 19,520 |
| arbitrary precision | `record_array` | 4096 | 3,080,192 (2,920,896) | 159,296 |
| preserve order | `record_array` | 512 | 288,256 (268,736) | 19,520 |
| preserve order | `record_array` | 4096 | 2,306,048 (2,146,752) | 159,296 |
| combined | `record_array` | 512 | 288,768 (269,248) | 19,520 |
| combined | `record_array` | 4096 | 2,310,144 (2,150,848) | 159,296 |

These differences cross the 20% screening threshold for the wide-object
fixtures, but they do not identify which allocations are removable. The
record-array differences are smaller and the control retains a different
result. Therefore the measurement does not establish a 20% reducible
duplicate-materialization cost or an equivalent safe decode path.

For latency context, the full Criterion command was run three times, with its
default 100 samples and `--noplot`:

```bash
cargo bench --locked --bench unique_decode_bench -- unique_decode_structural --noplot
```

The following are the center estimates and 95% confidence intervals for
unlimited, fresh, 4096-item cases (ms):

| Case | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| `wide_object` / `unique_typed` | 8.265 (7.613–8.978) | 6.249 (5.574–7.068) | 4.146 (4.105–4.192) |
| `wide_object` / `unique_seed` | 7.471 (6.876–8.132) | 30.671 (28.019–33.431) | 4.253 (4.199–4.310) |
| `record_array` / `unique_typed` | 5.356 (5.060–5.686) | 11.688 (10.707–12.724) | 3.546 (3.486–3.607) |
| `record_array` / `unique_seed` | 5.438 (5.182–5.711) | 6.498 (5.980–7.084) | 3.496 (3.448–3.545) |

The recorded 1-minute load average was about 10.78–19.65 during run 1, 10–33
during run 2, and about 26 at the start of run 3 before declining. The runs
also saw long-sample warnings and outliers; several comparisons changed
substantially between runs. Treat these timings as descriptive only. Since
neither the peak counter nor the latency samples
show an attributable, safely removable duplicate-materialization cost, this
check does not authorize a production-path optimization. A future proposal
would need a concrete consuming decode design and evidence for equivalent
number handling, error behavior, object ordering, and bounded failure-path
destruction before implementation.

The environment was Ubuntu 24.04.4 on Linux 7.0.0-38-generic, x86_64, Intel
Core i5-9600K (six cores), Rust 1.94.0, Criterion 0.8.2, and `serde_json`
1.0.154, using the system allocator. `Cargo.lock` SHA-256 was
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`. The
allocation counter self-check covered allocation/free, a free outside the
measurement window, realloc growth and shrinkage, and its byte-request
accounting. It is a benchmark diagnostic, not a production API.
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

### JCS stack growth after object-table reclamation (2026-10-29)

The internal `test_diagnoses_stack_growth_after_reclaiming_wide_table` fixture
in `src/encode/internal/jcs_working_memory.rs` exercises real reserve, push,
release, and later stack growth operations with
`jcs-allocation-diagnostics`. For each target depth (128, 512, 2048), it
creates a stack with spare capacity, reserves a wide member table whose
requested bytes exceed the budget remaining alongside that capacity, verifies
the stack replacement/reclamation, releases the table, snapshots diagnostic
counters, and then grows a deep array branch. A no-reclaim control starts from
the same stack capacity and length and uses the same budget. Budgets are
`depth * size_of::<[u64; 4]>() + 1024` bytes; the table width is derived from
the difference between that budget and the original stack capacity, using
`size_of::<[u64; 3]>()`. Both scenarios fit within their budgets.

The old-policy totals replay the exact-growth behavior in base revision
`48c4be653c3e342342327f46eb9e60b620e5a264`. Since the diagnostic fixture was
added by this change, the replay used the fixture here with the post-reclaim
high-water assignment temporarily set to `usize::MAX`, which matches the base
policy's `stack_growth_is_exact = true`. The geometric-growth assertion was
disabled for this capture so all three depths could be reported. It ran with
`rustc 1.94.0 (4a4ef493e 2026-03-02)` and the locked dependency set. Complete
output:

```text
JCS_STACK_RECLAIM depth=128 budget=5120 live_stack_bytes=256 peak_with_table=4888 peak_after_deep_branch=4888 reclaim_replacements=1 post_growths=120 post_replacements=0 post_requested_bytes=263040 control_growths=4 control_requested_bytes=8128
JCS_STACK_RECLAIM depth=512 budget=17408 live_stack_bytes=256 peak_with_table=17176 peak_after_deep_branch=17176 reclaim_replacements=1 post_growths=504 post_replacements=0 post_requested_bytes=4201344 control_growths=6 control_requested_bytes=32704
JCS_STACK_RECLAIM depth=2048 budget=66560 live_stack_bytes=256 peak_with_table=66328 peak_after_deep_branch=66328 reclaim_replacements=1 post_growths=2040 post_replacements=0 post_requested_bytes=67140480 control_growths=8 control_requested_bytes=131008
```

A separate run with the geometric-growth assertion enabled stopped at depth
128, observed 120 post-reclaim growths, and failed the bound. Both captures
used the command below:

```bash
cargo test --locked --lib --features 'jcs jcs-allocation-diagnostics' encode::internal::jcs_working_memory::tests -- --nocapture
```

The revised run starts from 8 live values with spare capacity and uses the
same start for its no-reclaim control. Its high-water mark allows geometric
growth only beyond previously requested depths. Peak values below are from the
revised run. No Criterion or wall-clock measurement was taken.

| Depth | Budget (B) | Revised peak with table (B) | Revised peak after deep branch (B) | Reclaim replacements | Post-reclaim growths, old / revised | Post-reclaim requested bytes, old / revised (B) | No-reclaim growths, revised |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 128 | 5,120 | 4,888 | 4,888 | 1 | 120 / 5 | 263,040 / 8,928 | 4 |
| 512 | 17,408 | 17,176 | 17,408 | 1 | 504 / 7 | 4,201,344 / 35,552 | 6 |
| 2048 | 66,560 | 66,328 | 66,560 | 1 | 2,040 / 9 | 67,140,480 / 140,000 | 8 |

The old policy therefore showed growth counts proportional to the added depth
after reclaim. With the high-water rule, the measured counts are logarithmic
over these depths and remain close to the same-start no-reclaim control. The
requested-byte sums count requested capacities across successful growth
events; they are not bytes copied, allocator traffic, live memory, or elapsed
time. The peak column is the tracked stack plus member-table capacity observed
by this internal fixture. The finite deep-branch peak reaches, but does not
exceed, its budget. The existing exact-boundary, finite-growth, gradually
wider-sibling, and ancestor-table preservation regressions passed with the
revised policy.

Reproduce with:

```bash
cargo test --locked --lib --features 'jcs jcs-allocation-diagnostics' encode::internal::jcs_working_memory::tests -- --nocapture
```

This bounded synthetic fixture establishes a capacity-growth regression and
its correction; it does not estimate end-to-end rendering latency or prove a
particular amount of physical copying.

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

## Duplicate-tail decode and iterative value-read frames (2026-10-29)

Measurements used Rust 1.94.0, `serde_json 1.0.154`, Criterion 0.8.2, the
system allocator, and an Intel Core i5-9600K on Linux 7.0.0-38-generic. The
host was shared and CPU affinity was not set. The unique-decode runs used
Criterion's default 100 samples; background crate builds overlapped part of
the collection, so latency pairs are observations rather than a controlled
speedup claim. Criterion uses the same benchmark IDs for `unique_seed` with
and without the JCS feature, so its automatic cross-run comparison can select
the other feature's latest sample. The table records the raw estimates and
reported intervals from matching feature runs instead.

Commands for duplicate-key decoding were:

```bash
cargo bench --locked --bench unique_decode_bench -- early_duplicate_tail
cargo bench --locked --bench unique_decode_bench --features jcs -- early_duplicate_tail
```

| Feature | Operation | Tail | Before estimate (reported interval) | After estimate (reported interval) |
| --- | --- | ---: | ---: | ---: |
| default | unique seed | 1 KiB | 1.9063 µs (1.8943–1.9187) | 1.5736 µs (1.5269–1.6295) |
| default | unique seed | 64 KiB | 66.148 µs (65.802–66.563) | 42.870 µs (41.908–43.934) |
| default | unique seed | 1 MiB | 1.4596 ms (1.4013–1.5223) | 533.18 µs (530.50–536.13) |
| `jcs` | unique seed | 1 KiB | 2.0086 µs (1.9379–2.0889) | 1.8600 µs (1.8432–1.8774) |
| `jcs` | unique seed | 64 KiB | 80.856 µs (77.954–83.903) | 72.543 µs (70.221–75.326) |
| `jcs` | unique seed | 1 MiB | 1.0482 ms (1.0435–1.0531) | 1.0292 ms (1.0234–1.0351) |
| `jcs` | JCS decode | 1 KiB | 1.7619 µs (1.7091–1.8288) | 1.8726 µs (1.8217–1.9334) |
| `jcs` | JCS decode | 64 KiB | 77.683 µs (75.496–80.351) | 63.781 µs (63.422–64.200) |
| `jcs` | JCS decode | 1 MiB | 1.0396 ms (1.0365–1.0428) | 1.0085 ms (1.0043–1.0132) |

The fixture has a duplicate near the start followed by valid string data. The
first complete admission pass remains in place, preserving syntax and budget
error precedence. The second construction pass now stops at the builder's
first materialization error. These samples do not impose a wall-clock
threshold; the JCS 1 KiB result is slightly slower and the measured samples
include shared-host noise.

The value-read allocation binary uses fixtures built before each measured
operation and writes Debug output to a discard sink or a sink that fails after
64 bytes. Reallocations are included in total requested bytes. Counts and
bytes measure allocation request traffic, not peak live memory or RSS.

```bash
cargo bench --locked --bench value_read_allocations
cargo bench --locked --bench value_read_allocations --features serde_json/preserve_order
```

For the default feature, owner and view results matched. Before the frame
change, comparison and Debug request traffic rose with width. After the change,
all three tested widths (32, 512, and 8192) reported the same requests:

| Shape / operation | Before at width 32 | Before at width 8192 | After, each width |
| --- | ---: | ---: | ---: |
| Array comparison (equal or unequal) | 2 / 528 B | 2 / 131,088 B | 1 / 320 B |
| Object comparison (equal or unequal) | 5 / 976 B | 13 / 262,096 B | 1 / 320 B |
| Array Debug (success or early writer failure) | 6 / 3,000 B | 14 / 786,360 B | 2 / 400 B |
| Object Debug (success or early writer failure) | 7 / 6,072 B | 15 / 1,572,792 B | 2 / 400 B |

With `serde_json/preserve_order`, the post-change results were constant across
the same widths: comparisons reported 1 allocation / 128 requested bytes and
Debug reported 2 / 120 bytes for both shapes and owner/view. No matching
pre-change preserve-order allocation sample was collected. At depths 8, 64,
and 256, default-feature comparison requests were 960, 9,920, and 40,640 B;
Debug requests were 2,320, 20,240, and 81,680 B. The preserve-order post-change
requests were respectively 384, 3,968, and 16,256 B for comparison, and 696,
6,072, and 24,504 B for Debug. This depth growth is consistent with the
explicit frame/action stacks; it also shows their per-depth allocation cost.

The focused time target was run with a 10-sample, 0.1-second warm-up and
measurement window:

```bash
cargo bench --locked --bench tree_bench -- json-value-read --warm-up-time 0.1 --measurement-time 0.1 --sample-size 10
```

Representative current estimates were 38.0 µs for equal 8192-element arrays,
676 µs for equal 8192-entry objects, 24–46 ns for a first-child mismatch, and
3.85 µs for depth-256 Debug to a discard sink. These short samples had outliers
and are descriptive only; the allocation results provide the width-scaling
evidence. No timing assertion is used.

### SafeJsonValue copy allocation baseline (2026-10-30)

This baseline measures `SafeJsonValue::copy_from` and `Clone` before any
production copy implementation change. Fixtures are created before the
measurement window. Each measured operation only creates the copy; equality
checks, `black_box`, and dropping the result happen after the window. The
benchmark first runs the allocation counter self-check. Object feature labels
are detected from `serde_json::Map` iteration order at runtime.

The two commands below were each run three times, sequentially. All 24 rows
for each feature configuration were identical across its three runs. The
default run 2 printed one Cargo package-cache lock wait; the other five runs
printed no lock wait. The printed `copy_from` and `clone` records were also
identical for every shape and size.

```bash
cargo bench --locked --bench value_clone_allocations
cargo bench --locked --bench value_clone_allocations --features serde_json/preserve_order
```

The CSV schema is
`VALUE_CLONE_ALLOC,shape,size,feature,operation,revision,allocation_count,allocated_request_bytes,reallocation_count,baseline_live_bytes,peak_live_bytes,end_live_bytes,peak_extra_bytes`.
In the table, `operation=copy_from,clone` means each operation emitted the
listed identical measurements. `size` is width for arrays/objects and depth
for the mixed chain. `end_live_bytes` includes the live fixture and benchmark
process state; subtracting `baseline_live_bytes` gives the net live-byte
change while the result is still alive. `peak_extra_bytes` is the maximum live
bytes above the baseline and therefore includes the copied result as well as
temporary allocations. `allocated_request_bytes` includes realloc request
sizes. These allocator diagnostics are not RSS or elapsed-time measurements.

| Feature | Shape | Size | Operation | Allocation count | Requested bytes | Reallocations | Baseline live B | Peak live B | End live B | Peak extra B |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default | array | 32 | copy_from, clone | 11 | 5,680 | 7 | 2,636 | 6,500 | 3,660 | 3,864 |
| default | array | 512 | copy_from, clone | 19 | 94,000 | 15 | 17,996 | 79,460 | 34,380 | 61,464 |
| default | array | 8192 | copy_from, clone | 27 | 1,507,120 | 23 | 263,756 | 1,246,820 | 525,900 | 983,064 |
| default | object_fixed_key | 32 | copy_from, clone | 45 | 8,072 | 4 | 5,380 | 12,532 | 8,924 | 7,152 |
| default | object_fixed_key | 512 | copy_from, clone | 608 | 132,400 | 8 | 63,948 | 180,068 | 122,700 | 116,120 |
| default | object_fixed_key | 8192 | copy_from, clone | 9,573 | 2,134,696 | 12 | 1,014,084 | 2,886,740 | 1,969,212 | 1,872,656 |
| default | object_escaped_non_ascii_key | 32 | copy_from, clone | 45 | 8,168 | 4 | 5,316 | 12,564 | 8,956 | 7,248 |
| default | object_escaped_non_ascii_key | 512 | copy_from, clone | 608 | 133,936 | 8 | 62,924 | 180,580 | 123,212 | 117,656 |
| default | object_escaped_non_ascii_key | 8192 | copy_from, clone | 9,573 | 2,159,272 | 12 | 997,700 | 2,894,932 | 1,977,404 | 1,897,232 |
| default | mixed_deep_chain | 8 | copy_from, clone | 29 | 4,528 | 3 | 4,292 | 7,996 | 7,356 | 3,704 |
| default | mixed_deep_chain | 64 | copy_from, clone | 200 | 35,832 | 6 | 23,052 | 50,892 | 47,564 | 27,840 |
| default | mixed_deep_chain | 256 | copy_from, clone | 778 | 143,160 | 8 | 87,372 | 197,964 | 185,420 | 110,592 |
| preserve_order | array | 32 | copy_from, clone | 11 | 11,760 | 7 | 3,916 | 11,620 | 6,220 | 7,704 |
| preserve_order | array | 512 | copy_from, clone | 19 | 196,080 | 15 | 38,476 | 161,380 | 75,340 | 122,904 |
| preserve_order | array | 8192 | copy_from, clone | 27 | 3,145,200 | 23 | 591,436 | 2,557,540 | 1,181,260 | 1,966,104 |
| preserve_order | object_fixed_key | 32 | copy_from, clone | 50 | 20,924 | 8 | 6,044 | 18,916 | 12,748 | 12,872 |
| preserve_order | object_fixed_key | 512 | copy_from, clone | 542 | 344,028 | 16 | 72,284 | 277,636 | 179,308 | 205,352 |
| preserve_order | object_fixed_key | 8192 | copy_from, clone | 8,234 | 5,512,732 | 24 | 1,132,124 | 4,417,156 | 2,844,268 | 3,285,032 |
| preserve_order | object_escaped_non_ascii_key | 32 | copy_from, clone | 50 | 21,020 | 8 | 5,980 | 18,948 | 12,780 | 12,968 |
| preserve_order | object_escaped_non_ascii_key | 512 | copy_from, clone | 542 | 345,564 | 16 | 71,260 | 278,148 | 179,820 | 206,888 |
| preserve_order | object_escaped_non_ascii_key | 8192 | copy_from, clone | 8,234 | 5,537,308 | 24 | 1,115,740 | 4,425,348 | 2,852,460 | 3,309,608 |
| preserve_order | mixed_deep_chain | 8 | copy_from, clone | 33 | 4,576 | 3 | 3,380 | 6,852 | 6,012 | 3,472 |
| preserve_order | mixed_deep_chain | 64 | copy_from, clone | 232 | 35,096 | 6 | 15,756 | 40,340 | 36,812 | 24,584 |
| preserve_order | mixed_deep_chain | 256 | copy_from, clone | 906 | 139,736 | 8 | 58,188 | 155,156 | 142,412 | 96,968 |

The sample revision was `db0a09232b634a26b87393628e20f83c4a47e709`; the
`Cargo.lock` SHA-256 was
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`.
`rustc -Vv` reported Rust 1.94.0, host `x86_64-unknown-linux-gnu`, LLVM
21.1.8. The platform was Ubuntu 24.04.4, Linux
`7.0.0-38-generic`, x86_64. The working tree was dirty during all samples:
`Cargo.toml`, `fuzz/Cargo.toml`, `fuzz/src/json_unique_decode_invariants.rs`,
`fuzz/src/lib.rs`, `fuzz/tests/json_extended_invariants_tests.rs`,
`src/encode/json_encoder.rs`, and untracked
`benches/value_clone_allocations.rs`, `fuzz/src/json_syntax_reference.rs`,
`fuzz/tests/json_syntax_reference_tests.rs`. These are the concurrent R01,
R03, and benchmark changes; `src/value/safe_json_value.rs` was unchanged.

At width 8192, the result remained live at the end of each measurement. The
net end-live changes and peak-extra measurements were 262,144 B / 983,064 B
for the default array, 955,128 B / 1,872,656 B for the default fixed-key
object, and 979,704 B / 1,897,232 B for its escaped/non-ASCII-key variant.
With `preserve_order`, they were 589,824 B / 1,966,104 B for the array,
1,712,144 B / 3,285,032 B for the fixed-key object, and 1,736,720 B /
3,309,608 B for its escaped/non-ASCII-key variant. Thus peak live bytes were
substantially above the final live result at the largest width in both map
configurations. This makes a T4 candidate experiment worth evaluating; it is
not evidence that a candidate will meet the 10% before/after threshold. No
production copy implementation was changed for this baseline, and no timing
assertion or RSS claim is made.

### SafeJsonValue copy candidate post-sample (2026-10-30)

After the baseline, the candidate replaced the eager task/output stacks with
depth-first iterator frames. The final frame stores an iterator, one optional
object key, and a boolean indicating whether its protected builder is active.
Active `SafeValueBuilder`s are kept in a separate LIFO vector. A builder is
created only when a container has siblings after the first child; single-child
chains therefore do not allocate or retain builder entries. On unwind, each
completed child remains a `SafeJsonValue`, each partial builder remains
protected, and nested builders are popped in frame order. No unsafe code was
added.

The same two commands were run three times each, sequentially, against this
final candidate. All 24 records per feature were identical across the three
runs; `copy_from` and `clone` matched for every record. The default runs had no
Cargo lock wait. The first preserve-order run printed one package-cache lock
wait before compiling; its two later runs did not. The candidate measurements
use the same schema and field definitions as the baseline table above. The
`operation` column below lists both identical operations.

| Feature | Shape | Size | Operation | Allocation count | Requested bytes | Reallocations | Baseline live B | Peak live B | End live B | Peak extra B |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default | array | 32 | copy_from, clone | 6 | 2,400 | 3 | 2,636 | 4,140 | 3,660 | 1,504 |
| default | array | 512 | copy_from, clone | 10 | 33,120 | 7 | 17,996 | 34,860 | 34,380 | 16,864 |
| default | array | 8192 | copy_from, clone | 14 | 524,640 | 11 | 263,756 | 526,380 | 525,900 | 262,624 |
| default | object_fixed_key | 32 | copy_from, clone | 39 | 4,024 | 0 | 5,380 | 9,404 | 8,924 | 4,024 |
| default | object_fixed_key | 512 | copy_from, clone | 598 | 59,232 | 0 | 63,948 | 123,180 | 122,700 | 59,232 |
| default | object_fixed_key | 8192 | copy_from, clone | 9,559 | 955,608 | 0 | 1,014,084 | 1,969,692 | 1,969,212 | 955,608 |
| default | object_escaped_non_ascii_key | 32 | copy_from, clone | 39 | 4,120 | 0 | 5,316 | 9,436 | 8,956 | 4,120 |
| default | object_escaped_non_ascii_key | 512 | copy_from, clone | 598 | 60,768 | 0 | 62,924 | 123,692 | 123,212 | 60,768 |
| default | object_escaped_non_ascii_key | 8192 | copy_from, clone | 9,559 | 980,184 | 0 | 997,700 | 1,977,884 | 1,977,404 | 980,184 |
| default | mixed_deep_chain | 8 | copy_from, clone | 14 | 4,120 | 1 | 4,292 | 8,060 | 7,356 | 3,768 |
| default | mixed_deep_chain | 64 | copy_from, clone | 101 | 35,424 | 4 | 23,052 | 53,196 | 47,564 | 30,144 |
| default | mixed_deep_chain | 256 | copy_from, clone | 391 | 142,752 | 6 | 87,372 | 207,948 | 185,420 | 120,576 |
| preserve_order | array | 32 | copy_from, clone | 6 | 4,736 | 3 | 3,916 | 6,636 | 6,220 | 2,720 |
| preserve_order | array | 512 | copy_from, clone | 10 | 73,856 | 7 | 38,476 | 75,756 | 75,340 | 37,280 |
| preserve_order | array | 8192 | copy_from, clone | 14 | 1,179,776 | 11 | 591,436 | 1,181,676 | 1,181,260 | 590,240 |
| preserve_order | object_fixed_key | 32 | copy_from, clone | 44 | 13,132 | 4 | 6,044 | 13,164 | 12,748 | 7,120 |
| preserve_order | object_fixed_key | 512 | copy_from, clone | 532 | 209,516 | 8 | 72,284 | 179,724 | 179,308 | 107,440 |
| preserve_order | object_fixed_key | 8192 | copy_from, clone | 8,220 | 3,350,700 | 12 | 1,132,124 | 2,844,684 | 2,844,268 | 1,712,560 |
| preserve_order | object_escaped_non_ascii_key | 32 | copy_from, clone | 44 | 13,228 | 4 | 5,980 | 13,196 | 12,780 | 7,216 |
| preserve_order | object_escaped_non_ascii_key | 512 | copy_from, clone | 532 | 211,052 | 8 | 71,260 | 180,236 | 179,820 | 108,976 |
| preserve_order | object_escaped_non_ascii_key | 8192 | copy_from, clone | 8,220 | 3,375,276 | 12 | 1,115,740 | 2,852,876 | 2,852,460 | 1,737,136 |
| preserve_order | mixed_deep_chain | 8 | copy_from, clone | 18 | 3,016 | 1 | 3,380 | 6,268 | 6,012 | 2,888 |
| preserve_order | mixed_deep_chain | 64 | copy_from, clone | 133 | 25,024 | 4 | 15,756 | 38,860 | 36,812 | 23,104 |
| preserve_order | mixed_deep_chain | 256 | copy_from, clone | 519 | 100,480 | 6 | 58,188 | 150,604 | 142,412 | 92,416 |

The post-sample revision remained
`db0a09232b634a26b87393628e20f83c4a47e709`, with `Cargo.lock` SHA-256
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a` and the
Rust/platform details recorded for the baseline above. The dirty worktree
during these samples contained `Cargo.toml`, both benchmark baseline documents,
the R01 fuzz files and `fuzz/Cargo.toml`, `src/encode/json_encoder.rs`,
`src/tests/j3/owner_tests.rs`, `src/value/internal/mod.rs`,
`src/value/safe_json_value.rs`, `tests/value/safe_json_value_tests.rs`, and
untracked `benches/value_clone_allocations.rs`,
`src/value/internal/json_value_copy_frame.rs`, and the new R01 fuzz module and
test. No changes were made to `Cargo.lock`.

At width 8192, baseline to final-candidate `peak_extra_bytes` changed from
983,064 to 262,624 B for the default array (73.3% lower), from 1,872,656 to
955,608 B for the fixed-key object (49.0% lower), and from 1,897,232 to
980,184 B for the escaped/non-ASCII-key object (48.3% lower). Under
`preserve_order`, the corresponding changes were 1,966,104 to 590,240 B
(70.0% lower), 3,285,032 to 1,712,560 B (47.9% lower), and 3,309,608 to
1,737,136 B (47.5% lower). Thus the candidate clears the design's 10% gate
for the 8192-item array and both object fixtures in both configurations.

The final mixed-chain measurements differ from baseline as follows:

| Feature | Depth | Baseline peak extra B | Candidate peak extra B | Change |
| --- | ---: | ---: | ---: | ---: |
| default | 8 | 3,704 | 3,768 | +1.7% |
| default | 64 | 27,840 | 30,144 | +8.3% |
| default | 256 | 110,592 | 120,576 | +9.0% (+9,984 B) |
| preserve_order | 8 | 3,472 | 2,888 | -16.8% |
| preserve_order | 64 | 24,584 | 23,104 | -6.0% |
| preserve_order | 256 | 96,968 | 92,416 | -4.7% |

The first frame variant stored a builder inline in every frame and raised
depth-256 peak-extra by 14.6% (default) and 12.2% (`preserve_order`). That
variant was not retained. The final compact frame removes the 8-byte builder
index and stores only a boolean; a separate builder stack contains entries
only for frames with unvisited siblings. For the tested single-child chain,
that vector remains allocation-free. The final default depth-256 peak is still
9.0% (9,984 B) above baseline, while allocated request volume is 142,752 B
versus 143,160 B at baseline; preserve-order deep-chain peaks are below
baseline. This remaining default deep-chain peak increase is reported as a
tradeoff, not hidden as an improvement. Given its size, the lower total request
volume, no unsafe code or semantic changes, and the 47.5–73.3% width gains in
both configurations, the candidate is retained under the design's conditions.
The tests directly compare copied values, compare source/copy key iteration
order, cover mixed depths 8/64/256, and exercise partial array and object
cleanup during injected unwind. No wall-clock threshold or RSS claim is made.


## RJ-01/RJ-02 allocation baseline before production candidates

- HEAD: `8aa4be27bb8babde6e10fc1284a0d83eee66a5da` (unchanged during T1).
- Rust: `rustc 1.94.0 (4a4ef493e 2026-03-02)`; host `x86_64-unknown-linux-gnu`; Cargo `1.94.0`.
- Host hardware: Intel Core i5-9600K @ 3.70 GHz, 6 CPUs; Linux kernel `7.0.0-38-generic`.
- `Cargo.lock` SHA-256: `65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`.
- Source SHA-256 (in listed order): Cargo.toml `1800a3b371d6b3626d739cbfde4d609ebe5bdbcb2131dd60313d1af5bc71bc17`; bench `a3d6d1bf3ec9c04a83bef207f791a193fe37fe326c482d9d6bb0ea11e605b2d7`; safe_value_builder `b043ec5e1e69b5be7da3eac64a0200a756e9fe42068750b7f6813855442cd9de`; json_value_serializer `4afc5f1af7f6ac4b9c704408ea48eb5d4f93f3083fc7f8ed0af2aaf233ad3cac`; json_value_compound `55dd5c054d5b4cc007b8b9fa0e829f0b9e558add4893402ba2b2173237e13d5e`; value_visitor `96fba144e005b1d1fe1d09194fc0f4537bbccbbb235be0c4cfcecf8877354fc1`.
- Captured dirty files: `Cargo.toml`, `benches/value_projection_allocations.rs`, `fuzz/src/json_unique_decode_invariants.rs`, `fuzz/tests/json_unique_decode_invariants_tests.rs`, `src/decode/json_decoder.rs`, `src/value/safe_json_value.rs`, `tests/encode/serde_compat/serde_json_compat_tests/json_private_tests.rs`. T1 did not edit the RJ-01/RJ-02 production source files listed above; the additional dirty files were already being edited by parallel T4/T5 work in this shared worktree.
- Fixture: widths 32/512/8192; sequence and explicit `serialize_bytes`; short keys `kNNNNN`; long keys use 128 ASCII `p` bytes plus a unique numeric suffix; array-chain depths 8/64/256. Project measures `JsonValueEncoder::encode_in_transaction`; bridge measures direct `serde_json` deserialization to `SafeJsonValue`. Fixture/input, expected values, and project transaction are prepared before measurement. The protected result remains alive through the measurement; value equality and transaction commit happen after it. Commit and result destruction are excluded consistently. For depth 256 only, `Deserializer::disable_recursion_limit` is required because `from_str`'s default recursion limit rejects the control fixture; it remains direct Serde deserialization and is validated outside the window.
- Allocation command, run three consecutive times per config: `cargo bench --locked --bench value_projection_allocations`; `cargo bench --locked --bench value_projection_allocations --features serde_json/preserve_order`. Each successful run emitted 24 rows. The first exploratory run failed on serde_json's default recursion limit at chain depth 256 and is excluded; the fixed run uses the explicit unbounded-depth deserializer for that control case. A compile attempt also caught and fixed a benchmark-only equality type mismatch; no production files changed.
- Allocation values are successful allocator request counts/bytes, realloc requests, and live/peak requested bytes; these are allocator-request diagnostics, not RSS. Across all three runs per feature, each row was identical.


The measured project and bridge operations use identical fixture definitions in each feature configuration. Each of the six complete runs below emitted 24 records.

### default allocation samples

| Round | Shape | Size | Operation | Allocations | Requested B | Reallocations | Baseline live B | Peak live B | End live B | Peak extra B |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | sequence | 32 | project | 5 | 2944 | 3 | 3054 | 5102 | 4078 | 2048 |
| 1 | bytes | 32 | project | 5 | 2944 | 3 | 4110 | 6158 | 5134 | 2048 |
| 1 | object_short | 32 | project | 75 | 6928 | 0 | 6990 | 13918 | 10438 | 6928 |
| 1 | object_short | 32 | bridge | 78 | 8496 | 3 | 7502 | 14430 | 10950 | 6928 |
| 1 | object_long_prefix | 32 | project | 75 | 15120 | 0 | 18318 | 33438 | 25862 | 15120 |
| 1 | object_long_prefix | 32 | bridge | 78 | 16688 | 3 | 26510 | 41630 | 34054 | 15120 |
| 1 | sequence | 512 | project | 9 | 49024 | 7 | 18414 | 51182 | 34798 | 32768 |
| 1 | bytes | 512 | project | 9 | 49024 | 7 | 35310 | 68078 | 51694 | 32768 |
| 1 | object_short | 512 | project | 1193 | 113536 | 0 | 70222 | 183758 | 127438 | 113536 |
| 1 | object_short | 512 | bridge | 1200 | 141984 | 7 | 78414 | 191950 | 135630 | 113536 |
| 1 | object_long_prefix | 512 | project | 1193 | 244608 | 0 | 240302 | 484910 | 363054 | 244608 |
| 1 | object_long_prefix | 512 | bridge | 1200 | 273056 | 7 | 371374 | 615982 | 494126 | 244608 |
| 1 | sequence | 8192 | project | 16 | 1015680 | 14 | 264174 | 788462 | 526318 | 524288 |
| 1 | bytes | 8192 | project | 16 | 1015680 | 14 | 534510 | 1058798 | 796654 | 524288 |
| 1 | object_short | 8192 | project | 19118 | 2240784 | 3 | 1077566 | 2916942 | 2008118 | 1839376 |
| 1 | object_short | 8192 | bridge | 19126 | 2297904 | 11 | 1208638 | 3048014 | 2139190 | 1839376 |
| 1 | object_long_prefix | 8192 | project | 19118 | 4337936 | 3 | 3783310 | 7719838 | 5762438 | 3936528 |
| 1 | object_long_prefix | 8192 | bridge | 19126 | 4395056 | 11 | 5880462 | 9816990 | 7859590 | 3936528 |
| 1 | chain | 8 | project | 16 | 1280 | 0 | 2286 | 3342 | 3310 | 1056 |
| 1 | chain | 8 | bridge | 8 | 1024 | 0 | 2414 | 3438 | 3438 | 1024 |
| 1 | chain | 64 | project | 128 | 10240 | 0 | 4078 | 12302 | 12270 | 8224 |
| 1 | chain | 64 | bridge | 64 | 8192 | 0 | 4334 | 12526 | 12526 | 8192 |
| 1 | chain | 256 | project | 512 | 40960 | 0 | 10222 | 43022 | 42990 | 32800 |
| 1 | chain | 256 | bridge | 256 | 32768 | 0 | 11246 | 44014 | 44014 | 32768 |
| 2 | sequence | 32 | project | 5 | 2944 | 3 | 3054 | 5102 | 4078 | 2048 |
| 2 | bytes | 32 | project | 5 | 2944 | 3 | 4110 | 6158 | 5134 | 2048 |
| 2 | object_short | 32 | project | 75 | 6928 | 0 | 6990 | 13918 | 10438 | 6928 |
| 2 | object_short | 32 | bridge | 78 | 8496 | 3 | 7502 | 14430 | 10950 | 6928 |
| 2 | object_long_prefix | 32 | project | 75 | 15120 | 0 | 18318 | 33438 | 25862 | 15120 |
| 2 | object_long_prefix | 32 | bridge | 78 | 16688 | 3 | 26510 | 41630 | 34054 | 15120 |
| 2 | sequence | 512 | project | 9 | 49024 | 7 | 18414 | 51182 | 34798 | 32768 |
| 2 | bytes | 512 | project | 9 | 49024 | 7 | 35310 | 68078 | 51694 | 32768 |
| 2 | object_short | 512 | project | 1193 | 113536 | 0 | 70222 | 183758 | 127438 | 113536 |
| 2 | object_short | 512 | bridge | 1200 | 141984 | 7 | 78414 | 191950 | 135630 | 113536 |
| 2 | object_long_prefix | 512 | project | 1193 | 244608 | 0 | 240302 | 484910 | 363054 | 244608 |
| 2 | object_long_prefix | 512 | bridge | 1200 | 273056 | 7 | 371374 | 615982 | 494126 | 244608 |
| 2 | sequence | 8192 | project | 16 | 1015680 | 14 | 264174 | 788462 | 526318 | 524288 |
| 2 | bytes | 8192 | project | 16 | 1015680 | 14 | 534510 | 1058798 | 796654 | 524288 |
| 2 | object_short | 8192 | project | 19118 | 2240784 | 3 | 1077566 | 2916942 | 2008118 | 1839376 |
| 2 | object_short | 8192 | bridge | 19126 | 2297904 | 11 | 1208638 | 3048014 | 2139190 | 1839376 |
| 2 | object_long_prefix | 8192 | project | 19118 | 4337936 | 3 | 3783310 | 7719838 | 5762438 | 3936528 |
| 2 | object_long_prefix | 8192 | bridge | 19126 | 4395056 | 11 | 5880462 | 9816990 | 7859590 | 3936528 |
| 2 | chain | 8 | project | 16 | 1280 | 0 | 2286 | 3342 | 3310 | 1056 |
| 2 | chain | 8 | bridge | 8 | 1024 | 0 | 2414 | 3438 | 3438 | 1024 |
| 2 | chain | 64 | project | 128 | 10240 | 0 | 4078 | 12302 | 12270 | 8224 |
| 2 | chain | 64 | bridge | 64 | 8192 | 0 | 4334 | 12526 | 12526 | 8192 |
| 2 | chain | 256 | project | 512 | 40960 | 0 | 10222 | 43022 | 42990 | 32800 |
| 2 | chain | 256 | bridge | 256 | 32768 | 0 | 11246 | 44014 | 44014 | 32768 |
| 3 | sequence | 32 | project | 5 | 2944 | 3 | 3054 | 5102 | 4078 | 2048 |
| 3 | bytes | 32 | project | 5 | 2944 | 3 | 4110 | 6158 | 5134 | 2048 |
| 3 | object_short | 32 | project | 75 | 6928 | 0 | 6990 | 13918 | 10438 | 6928 |
| 3 | object_short | 32 | bridge | 78 | 8496 | 3 | 7502 | 14430 | 10950 | 6928 |
| 3 | object_long_prefix | 32 | project | 75 | 15120 | 0 | 18318 | 33438 | 25862 | 15120 |
| 3 | object_long_prefix | 32 | bridge | 78 | 16688 | 3 | 26510 | 41630 | 34054 | 15120 |
| 3 | sequence | 512 | project | 9 | 49024 | 7 | 18414 | 51182 | 34798 | 32768 |
| 3 | bytes | 512 | project | 9 | 49024 | 7 | 35310 | 68078 | 51694 | 32768 |
| 3 | object_short | 512 | project | 1193 | 113536 | 0 | 70222 | 183758 | 127438 | 113536 |
| 3 | object_short | 512 | bridge | 1200 | 141984 | 7 | 78414 | 191950 | 135630 | 113536 |
| 3 | object_long_prefix | 512 | project | 1193 | 244608 | 0 | 240302 | 484910 | 363054 | 244608 |
| 3 | object_long_prefix | 512 | bridge | 1200 | 273056 | 7 | 371374 | 615982 | 494126 | 244608 |
| 3 | sequence | 8192 | project | 16 | 1015680 | 14 | 264174 | 788462 | 526318 | 524288 |
| 3 | bytes | 8192 | project | 16 | 1015680 | 14 | 534510 | 1058798 | 796654 | 524288 |
| 3 | object_short | 8192 | project | 19118 | 2240784 | 3 | 1077566 | 2916942 | 2008118 | 1839376 |
| 3 | object_short | 8192 | bridge | 19126 | 2297904 | 11 | 1208638 | 3048014 | 2139190 | 1839376 |
| 3 | object_long_prefix | 8192 | project | 19118 | 4337936 | 3 | 3783310 | 7719838 | 5762438 | 3936528 |
| 3 | object_long_prefix | 8192 | bridge | 19126 | 4395056 | 11 | 5880462 | 9816990 | 7859590 | 3936528 |
| 3 | chain | 8 | project | 16 | 1280 | 0 | 2286 | 3342 | 3310 | 1056 |
| 3 | chain | 8 | bridge | 8 | 1024 | 0 | 2414 | 3438 | 3438 | 1024 |
| 3 | chain | 64 | project | 128 | 10240 | 0 | 4078 | 12302 | 12270 | 8224 |
| 3 | chain | 64 | bridge | 64 | 8192 | 0 | 4334 | 12526 | 12526 | 8192 |
| 3 | chain | 256 | project | 512 | 40960 | 0 | 10222 | 43022 | 42990 | 32800 |
| 3 | chain | 256 | bridge | 256 | 32768 | 0 | 11246 | 44014 | 44014 | 32768 |

### preserve_order allocation samples

| Round | Shape | Size | Operation | Allocations | Requested B | Reallocations | Baseline live B | Peak live B | End live B | Peak extra B |
| ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | sequence | 32 | project | 5 | 6624 | 3 | 4334 | 8942 | 6638 | 4608 |
| 1 | bytes | 32 | project | 5 | 6624 | 3 | 6670 | 11278 | 8974 | 4608 |
| 1 | object_short | 32 | project | 80 | 17380 | 4 | 10846 | 22214 | 17454 | 11368 |
| 1 | object_short | 32 | bridge | 83 | 20068 | 7 | 11358 | 22726 | 17966 | 11368 |
| 1 | object_long_prefix | 32 | project | 80 | 25572 | 4 | 23470 | 43030 | 34174 | 19560 |
| 1 | object_long_prefix | 32 | bridge | 83 | 28260 | 7 | 31662 | 51222 | 42366 | 19560 |
| 1 | sequence | 512 | project | 9 | 110304 | 7 | 38894 | 112622 | 75758 | 73728 |
| 1 | bytes | 512 | project | 9 | 110304 | 7 | 76270 | 149998 | 113134 | 73728 |
| 1 | object_short | 512 | project | 1127 | 284364 | 8 | 142846 | 325134 | 248334 | 182288 |
| 1 | object_short | 512 | bridge | 1134 | 333132 | 15 | 151038 | 333326 | 256526 | 182288 |
| 1 | object_long_prefix | 512 | project | 1127 | 415436 | 8 | 344590 | 657950 | 515614 | 313360 |
| 1 | object_long_prefix | 512 | bridge | 1134 | 464204 | 15 | 475662 | 789022 | 646686 | 313360 |
| 1 | sequence | 8192 | project | 16 | 2285280 | 14 | 591854 | 1771502 | 1181678 | 1179648 |
| 1 | bytes | 8192 | project | 16 | 2285280 | 14 | 1189870 | 2369518 | 1779694 | 1179648 |
| 1 | object_short | 8192 | project | 17779 | 5250340 | 15 | 2254846 | 5178918 | 3942414 | 2924072 |
| 1 | object_short | 8192 | bridge | 17787 | 5348260 | 23 | 2385918 | 5309990 | 4073486 | 2924072 |
| 1 | object_long_prefix | 8192 | project | 17779 | 7347492 | 15 | 5482510 | 10503734 | 8218654 | 5021224 |
| 1 | object_long_prefix | 8192 | bridge | 17787 | 7445412 | 23 | 7579662 | 12600886 | 10315806 | 5021224 |
| 1 | chain | 8 | project | 16 | 2880 | 0 | 2606 | 4982 | 4910 | 2376 |
| 1 | chain | 8 | bridge | 8 | 2304 | 0 | 2734 | 5038 | 5038 | 2304 |
| 1 | chain | 64 | project | 128 | 23040 | 0 | 6638 | 25142 | 25070 | 18504 |
| 1 | chain | 64 | bridge | 64 | 18432 | 0 | 6894 | 25326 | 25326 | 18432 |
| 1 | chain | 256 | project | 512 | 92160 | 0 | 20462 | 94262 | 94190 | 73800 |
| 1 | chain | 256 | bridge | 256 | 73728 | 0 | 21486 | 95214 | 95214 | 73728 |
| 2 | sequence | 32 | project | 5 | 6624 | 3 | 4334 | 8942 | 6638 | 4608 |
| 2 | bytes | 32 | project | 5 | 6624 | 3 | 6670 | 11278 | 8974 | 4608 |
| 2 | object_short | 32 | project | 80 | 17380 | 4 | 10846 | 22214 | 17454 | 11368 |
| 2 | object_short | 32 | bridge | 83 | 20068 | 7 | 11358 | 22726 | 17966 | 11368 |
| 2 | object_long_prefix | 32 | project | 80 | 25572 | 4 | 23470 | 43030 | 34174 | 19560 |
| 2 | object_long_prefix | 32 | bridge | 83 | 28260 | 7 | 31662 | 51222 | 42366 | 19560 |
| 2 | sequence | 512 | project | 9 | 110304 | 7 | 38894 | 112622 | 75758 | 73728 |
| 2 | bytes | 512 | project | 9 | 110304 | 7 | 76270 | 149998 | 113134 | 73728 |
| 2 | object_short | 512 | project | 1127 | 284364 | 8 | 142846 | 325134 | 248334 | 182288 |
| 2 | object_short | 512 | bridge | 1134 | 333132 | 15 | 151038 | 333326 | 256526 | 182288 |
| 2 | object_long_prefix | 512 | project | 1127 | 415436 | 8 | 344590 | 657950 | 515614 | 313360 |
| 2 | object_long_prefix | 512 | bridge | 1134 | 464204 | 15 | 475662 | 789022 | 646686 | 313360 |
| 2 | sequence | 8192 | project | 16 | 2285280 | 14 | 591854 | 1771502 | 1181678 | 1179648 |
| 2 | bytes | 8192 | project | 16 | 2285280 | 14 | 1189870 | 2369518 | 1779694 | 1179648 |
| 2 | object_short | 8192 | project | 17779 | 5250340 | 15 | 2254846 | 5178918 | 3942414 | 2924072 |
| 2 | object_short | 8192 | bridge | 17787 | 5348260 | 23 | 2385918 | 5309990 | 4073486 | 2924072 |
| 2 | object_long_prefix | 8192 | project | 17779 | 7347492 | 15 | 5482510 | 10503734 | 8218654 | 5021224 |
| 2 | object_long_prefix | 8192 | bridge | 17787 | 7445412 | 23 | 7579662 | 12600886 | 10315806 | 5021224 |
| 2 | chain | 8 | project | 16 | 2880 | 0 | 2606 | 4982 | 4910 | 2376 |
| 2 | chain | 8 | bridge | 8 | 2304 | 0 | 2734 | 5038 | 5038 | 2304 |
| 2 | chain | 64 | project | 128 | 23040 | 0 | 6638 | 25142 | 25070 | 18504 |
| 2 | chain | 64 | bridge | 64 | 18432 | 0 | 6894 | 25326 | 25326 | 18432 |
| 2 | chain | 256 | project | 512 | 92160 | 0 | 20462 | 94262 | 94190 | 73800 |
| 2 | chain | 256 | bridge | 256 | 73728 | 0 | 21486 | 95214 | 95214 | 73728 |
| 3 | sequence | 32 | project | 5 | 6624 | 3 | 4334 | 8942 | 6638 | 4608 |
| 3 | bytes | 32 | project | 5 | 6624 | 3 | 6670 | 11278 | 8974 | 4608 |
| 3 | object_short | 32 | project | 80 | 17380 | 4 | 10846 | 22214 | 17454 | 11368 |
| 3 | object_short | 32 | bridge | 83 | 20068 | 7 | 11358 | 22726 | 17966 | 11368 |
| 3 | object_long_prefix | 32 | project | 80 | 25572 | 4 | 23470 | 43030 | 34174 | 19560 |
| 3 | object_long_prefix | 32 | bridge | 83 | 28260 | 7 | 31662 | 51222 | 42366 | 19560 |
| 3 | sequence | 512 | project | 9 | 110304 | 7 | 38894 | 112622 | 75758 | 73728 |
| 3 | bytes | 512 | project | 9 | 110304 | 7 | 76270 | 149998 | 113134 | 73728 |
| 3 | object_short | 512 | project | 1127 | 284364 | 8 | 142846 | 325134 | 248334 | 182288 |
| 3 | object_short | 512 | bridge | 1134 | 333132 | 15 | 151038 | 333326 | 256526 | 182288 |
| 3 | object_long_prefix | 512 | project | 1127 | 415436 | 8 | 344590 | 657950 | 515614 | 313360 |
| 3 | object_long_prefix | 512 | bridge | 1134 | 464204 | 15 | 475662 | 789022 | 646686 | 313360 |
| 3 | sequence | 8192 | project | 16 | 2285280 | 14 | 591854 | 1771502 | 1181678 | 1179648 |
| 3 | bytes | 8192 | project | 16 | 2285280 | 14 | 1189870 | 2369518 | 1779694 | 1179648 |
| 3 | object_short | 8192 | project | 17779 | 5250340 | 15 | 2254846 | 5178918 | 3942414 | 2924072 |
| 3 | object_short | 8192 | bridge | 17787 | 5348260 | 23 | 2385918 | 5309990 | 4073486 | 2924072 |
| 3 | object_long_prefix | 8192 | project | 17779 | 7347492 | 15 | 5482510 | 10503734 | 8218654 | 5021224 |
| 3 | object_long_prefix | 8192 | bridge | 17787 | 7445412 | 23 | 7579662 | 12600886 | 10315806 | 5021224 |
| 3 | chain | 8 | project | 16 | 2880 | 0 | 2606 | 4982 | 4910 | 2376 |
| 3 | chain | 8 | bridge | 8 | 2304 | 0 | 2734 | 5038 | 5038 | 2304 |
| 3 | chain | 64 | project | 128 | 23040 | 0 | 6638 | 25142 | 25070 | 18504 |
| 3 | chain | 64 | bridge | 64 | 18432 | 0 | 6894 | 25326 | 25326 | 18432 |
| 3 | chain | 256 | project | 512 | 92160 | 0 | 20462 | 94262 | 94190 | 73800 |
| 3 | chain | 256 | bridge | 256 | 73728 | 0 | 21486 | 95214 | 95214 | 73728 |

### RJ-02 latency baseline attempt (default feature only)

Command: `cargo bench --locked --bench decoder_bench -- safe_value_bridge` (full Criterion sampling; 100 samples per case, no `--quick`). The command completed all 36 benchmark cases. The table records Criterion's center estimate and 95% confidence interval as emitted, plus outlier counts:

| Benchmark | Lower (95% CI) | Estimate | Upper (95% CI) | Outliers |
| --- | ---: | ---: | ---: | ---: |
| safe_value_bridge/serde/short/unique/32 | 28.403 µs | 34.276 µs | 40.419 µs | 1/100 |
| safe_value_bridge/json_decoder/short/unique/32 | 105.62 µs | 113.63 µs | 121.94 µs | 3/100 |
| safe_value_bridge/serde/short/duplicate_early/32 | 2.0403 µs | 2.3053 µs | 2.5988 µs | 3/100 |
| safe_value_bridge/json_decoder/short/duplicate_early/32 | 2.8726 µs | 3.3246 µs | 3.8275 µs | 4/100 |
| safe_value_bridge/serde/short/duplicate_tail/32 | 34.316 µs | 39.754 µs | 45.415 µs | 4/100 |
| safe_value_bridge/json_decoder/short/duplicate_tail/32 | 92.754 µs | 107.33 µs | 122.21 µs | 3/100 |
| safe_value_bridge/serde/long_prefix/unique/32 | 62.565 µs | 68.827 µs | 74.981 µs | 4/100 |
| safe_value_bridge/json_decoder/long_prefix/unique/32 | 42.971 µs | 48.378 µs | 54.588 µs | 4/100 |
| safe_value_bridge/serde/long_prefix/duplicate_early/32 | 1.0768 µs | 1.4664 µs | 2.0863 µs | 6/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_early/32 | 2.3808 µs | 3.1282 µs | 4.0385 µs | 8/100 |
| safe_value_bridge/serde/long_prefix/duplicate_tail/32 | 20.688 µs | 31.342 µs | 42.731 µs | 13/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_tail/32 | 102.19 µs | 113.38 µs | 125.07 µs | 3/100 |
| safe_value_bridge/serde/short/unique/512 | 589.77 µs | 707.75 µs | 840.34 µs | 5/100 |
| safe_value_bridge/json_decoder/short/unique/512 | 1.6295 ms | 1.8767 ms | 2.1436 ms | 1/100 |
| safe_value_bridge/serde/short/duplicate_early/512 | 1.1712 µs | 1.4728 µs | 1.8089 µs | 9/100 |
| safe_value_bridge/json_decoder/short/duplicate_early/512 | 2.1705 µs | 2.5048 µs | 2.8987 µs | 2/100 |
| safe_value_bridge/serde/short/duplicate_tail/512 | 424.90 µs | 487.92 µs | 560.31 µs | 2/100 |
| safe_value_bridge/json_decoder/short/duplicate_tail/512 | 1.0293 ms | 1.2437 ms | 1.4786 ms | 3/100 |
| safe_value_bridge/serde/long_prefix/unique/512 | 525.09 µs | 636.48 µs | 766.18 µs | 5/100 |
| safe_value_bridge/json_decoder/long_prefix/unique/512 | 1.3005 ms | 1.4953 ms | 1.7058 ms | 4/100 |
| safe_value_bridge/serde/long_prefix/duplicate_early/512 | 1.5845 µs | 1.8823 µs | 2.1821 µs | 1/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_early/512 | 6.7938 µs | 7.6372 µs | 8.4387 µs | 2/100 |
| safe_value_bridge/serde/long_prefix/duplicate_tail/512 | 371.75 µs | 428.25 µs | 487.73 µs | 1/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_tail/512 | 1.4121 ms | 1.6670 ms | 1.9290 ms | 5/100 |
| safe_value_bridge/serde/short/unique/4096 | 7.9057 ms | 8.6422 ms | 9.4354 ms | 1/100 |
| safe_value_bridge/json_decoder/short/unique/4096 | 10.839 ms | 11.765 ms | 12.691 ms | 4/100 |
| safe_value_bridge/serde/short/duplicate_early/4096 | 1.0311 µs | 1.1389 µs | 1.2632 µs | 4/100 |
| safe_value_bridge/json_decoder/short/duplicate_early/4096 | 5.3698 µs | 6.0881 µs | 6.9337 µs | 2/100 |
| safe_value_bridge/serde/short/duplicate_tail/4096 | 6.5620 ms | 7.1397 ms | 7.7525 ms | 2/100 |
| safe_value_bridge/json_decoder/short/duplicate_tail/4096 | 5.8590 ms | 6.4975 ms | 7.1895 ms | 2/100 |
| safe_value_bridge/serde/long_prefix/unique/4096 | 5.5580 ms | 6.4765 ms | 7.4909 ms | 6/100 |
| safe_value_bridge/json_decoder/long_prefix/unique/4096 | 8.9081 ms | 10.509 ms | 12.302 ms | 11/100 |
| safe_value_bridge/serde/long_prefix/duplicate_early/4096 | 1.0592 µs | 1.1890 µs | 1.3269 µs | 1/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_early/4096 | 26.874 µs | 32.260 µs | 39.656 µs | 5/100 |
| safe_value_bridge/serde/long_prefix/duplicate_tail/4096 | 7.3674 ms | 8.4231 ms | 9.6464 ms | 2/100 |
| safe_value_bridge/json_decoder/long_prefix/duplicate_tail/4096 | 6.9676 ms | 8.4150 ms | 9.9817 ms | 7/100 |

Environment: same HEAD/toolchain/lockfile as above. During the run, the shared host had 23 logged-in users and load averages reached `47.13, 41.14, 30.70`; immediately after completion they were `28.71, 34.10, 30.86`. Other worktree CI, Rust compiler, and Miri processes were active, and several cases had high-severe outliers. Therefore these values are preserved as raw observations but are not a reliable RJ-02 latency baseline. The required `preserve_order` Criterion command was not run because host load remained high. RJ-02 latency remains unverified; do not use these measurements to decide adoption. Full raw output was retained during this task at `/tmp/rj-t1-criterion-default.log`.
### RJ-01 protected-array accumulation post-measurement (2026-10-30)

After T1, sequence, tuple-variant, and explicit `serialize_bytes` accumulation
use `SafeValueBuilder` directly. The six post runs used the same commands,
fixtures, request-counter window, and fields as the pre-measurement section
above: three default runs followed by three `serde_json/preserve_order` runs.
All 24 output records were identical within each configuration and across its
three rounds. The two width-8192 project fixtures each reduced
`peak_extra_bytes` by 50.0% in both configurations. The depth controls did not
increase; chain project controls decreased and bridge controls were unchanged.
These allocator-request measurements are not RSS.

The following raw rows show all measurement fields for the changed array
fixtures and the depth controls. The unchanged bridge rows are included to
show the complete control result. Round 1 represents three identical rounds.

| Round | Shape | Size | Operation | Feature | Allocations | Requested B | Reallocations | Baseline live B | Peak live B | End live B | Peak extra B |
| ---: | --- | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | sequence | 8192 | project | default | 4 | 491,520 | 3 | 264,488 | 526,632 | 526,632 | 262,144 |
| 1 | bytes | 8192 | project | default | 4 | 491,520 | 3 | 534,824 | 796,968 | 796,968 | 262,144 |
| 1 | chain | 8 | project | default | 8 | 256 | 0 | 2,600 | 2,856 | 2,856 | 256 |
| 1 | chain | 8 | bridge | default | 8 | 1,024 | 0 | 2,728 | 3,752 | 3,752 | 1,024 |
| 1 | chain | 64 | project | default | 64 | 2,048 | 0 | 4,392 | 6,440 | 6,440 | 2,048 |
| 1 | chain | 64 | bridge | default | 64 | 8,192 | 0 | 4,648 | 12,840 | 12,840 | 8,192 |
| 1 | chain | 256 | project | default | 256 | 8,192 | 0 | 10,536 | 18,728 | 18,728 | 8,192 |
| 1 | chain | 256 | bridge | default | 256 | 32,768 | 0 | 11,560 | 44,328 | 44,328 | 32,768 |
| 1 | sequence | 8192 | project | preserve_order | 4 | 1,105,920 | 3 | 592,168 | 1,181,992 | 1,181,992 | 589,824 |
| 1 | bytes | 8192 | project | preserve_order | 4 | 1,105,920 | 3 | 1,190,184 | 1,780,008 | 1,780,008 | 589,824 |
| 1 | chain | 8 | project | preserve_order | 8 | 576 | 0 | 2,920 | 3,496 | 3,496 | 576 |
| 1 | chain | 8 | bridge | preserve_order | 8 | 2,304 | 0 | 3,048 | 5,352 | 5,352 | 2,304 |
| 1 | chain | 64 | project | preserve_order | 64 | 4,608 | 0 | 6,952 | 11,560 | 11,560 | 4,608 |
| 1 | chain | 64 | bridge | preserve_order | 64 | 18,432 | 0 | 7,208 | 25,640 | 25,640 | 18,432 |
| 1 | chain | 256 | project | preserve_order | 256 | 18,432 | 0 | 20,776 | 39,208 | 39,208 | 18,432 |
| 1 | chain | 256 | bridge | preserve_order | 256 | 73,728 | 0 | 21,800 | 95,528 | 95,528 | 73,728 |

For exact `peak_extra_bytes` comparisons, T1 to T2 was:

| Feature | Shape / operation | T1 B | T2 B | Change |
| --- | --- | ---: | ---: | ---: |
| default | sequence / project, 8192 | 524,288 | 262,144 | -50.0% |
| default | bytes / project, 8192 | 524,288 | 262,144 | -50.0% |
| preserve_order | sequence / project, 8192 | 1,179,648 | 589,824 | -50.0% |
| preserve_order | bytes / project, 8192 | 1,179,648 | 589,824 | -50.0% |
| default | chain / project, depth 8 | 1,056 | 256 | -75.8% |
| default | chain / project, depth 64 | 8,224 | 2,048 | -75.1% |
| default | chain / project, depth 256 | 32,800 | 8,192 | -75.0% |
| default | chain / bridge, depth 8 | 1,024 | 1,024 | 0.0% |
| default | chain / bridge, depth 64 | 8,192 | 8,192 | 0.0% |
| default | chain / bridge, depth 256 | 32,768 | 32,768 | 0.0% |
| preserve_order | chain / project, depth 8 | 2,376 | 576 | -75.8% |
| preserve_order | chain / project, depth 64 | 18,504 | 4,608 | -75.1% |
| preserve_order | chain / project, depth 256 | 73,800 | 18,432 | -75.0% |
| preserve_order | chain / bridge, depth 8 | 2,304 | 2,304 | 0.0% |
| preserve_order | chain / bridge, depth 64 | 18,432 | 18,432 | 0.0% |
| preserve_order | chain / bridge, depth 256 | 73,728 | 73,728 | 0.0% |

The candidate meets the RJ-01 allocation gate in both map configurations and
all depth controls. T2 leaves `SafeValueBuilder::array_with_capacity` and
guarded sequence/tuple-variant accumulation in place for the next task; it
does not alter the map visitor targeted by RJ-02.

Post-measurement identity: HEAD `029c4dd0678d5506a3a9b7cd09ded9f96654bcab`;
`rustc 1.94.0 (4a4ef493e 2026-03-02)`; Cargo `1.94.0`; Cargo.lock SHA-256
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`.
The source hashes emitted by the benchmark were: Cargo.toml
`1800a3b371d6b3626d739cbfde4d609ebe5bdbcb2131dd60313d1af5bc71bc17`;
benchmark `a3d6d1bf3ec9c04a83bef207f791a193fe37fe326c482d9d6bb0ea11e605b2d7`;
SafeValueBuilder `eb1d7c7b07ab42e5dc5fd2518986b0f5621eef1446324d8922a2cdb133933d87`;
serializer `7c3664d1f4283743d77bb2279514f22b0660a361cf135099ae6d821f51f12e9c`;
compound `fc2aea2915fbb961140fd75e583124a882e41d236d01ad25c873f529523cbca0`;
and unchanged value visitor `96fba144e005b1d1fe1d09194fc0f4537bbccbbb235be0c4cfcecf8877354fc1`.
Additional T2 source hashes: compound state
`662d5e25e1e10744fab968cd097a7e3908311f20b721da51ce653d83267d08a4`;
encoder tests `0b628733f2cddd7c81168b05d4847a7163c4d07fd8b9fbadc6f6a603bd1a2121`;
owner tests `5ba9817fb210ee75c060cd04ded29bb9106df7058542072e429fef41dd928edb`.
The measured worktree also contained unrelated T4/T5 edits. The benchmark's
dirty-file metadata was: `doc/benchmark_baseline.md`,
`doc/benchmark_baseline.zh_CN.md`, `fuzz/src/json_unique_decode_invariants.rs`,
`fuzz/tests/json_unique_decode_invariants_tests.rs`, `src/decode/json_decoder.rs`,
`src/tests/j3/owner_tests.rs`, `src/value/internal/safe_value_builder.rs`,
`src/value/json_value_encoder/internal/compound_state.rs`,
`src/value/json_value_encoder/internal/json_value_compound.rs`,
`src/value/json_value_encoder/internal/json_value_serializer.rs`,
`src/value/safe_json_value.rs`,
`tests/encode/serde_compat/serde_json_compat_tests/json_private_tests.rs`, and
`tests/value/json_value_encoder_tests.rs`.

### RJ-02 object-bridge decision (2026-10-30)

The allocation pre-measurement above is available for both `default` and
`serde_json/preserve_order`, with three identical rounds per configuration.
However, the latency pre-measurement is not reliable: the completed default
Criterion run had severe shared-host load and high-severe outliers, and the
required `preserve_order` latency run was not collected. The raw default output
is retained above as an unreliable attempt, not as a usable latency baseline.

Accordingly, the RJ-02 production candidate was not adopted or performance
checked in this round. No conclusion is made that it would improve or regress
latency or allocation behavior. Reconsider it only after collecting reliable
`safe_value_bridge` latency pre-measurements for both configurations in a
low-interference environment. The focused object-bridge tests still verify
behavior, including that `{"a":0,"a":[}` reports the duplicate-key data
error before attempting to read the malformed duplicate value; this is a
correctness result, not performance evidence.
## Lexical array allocation baseline (RSJSON-002)

`lexical_array_allocations` measures arrays assembled by the lexical JSON value
consumer. Fixtures are built before measurement and include an empty array,
scalar arrays of 1/32/512/4096 items, record arrays of 32/512/4096 records, and
single-child chains of depth 8/64/256/4096. Chain measurements use
`SafeJsonValue`; validation walks each chain iteratively, and the owner remains
alive until the allocation window has closed.

Paths include `decode_value`, validated number-preserving decode, unique typed
and seed decode for scalar and record arrays, and `JcsDecoder::decode_value_str`
when the `jcs` feature is enabled. Every path runs unlimited and with finite
limits set to the exact source byte, node, root-inclusive depth, and sequence
item requirements of that fixture. Typed paths intentionally omit deep chains.
The Criterion `lexical_arrays` group uses the same inputs and paths and runs
without the counting allocator, with decoder construction and result destruction
inside the timed operation.

The captured CSV output reports allocation calls, requested bytes, realloc calls,
`peak_extra_bytes`, and `end_delta`. Requested bytes are cumulative allocator
requests, not RSS or physical peak memory. `peak_extra_bytes` follows the
existing allocator's requested-size live-byte accounting and is not a precise
physical peak. `end_delta` is measured while the decoded output remains alive;
it helps detect retained-capacity changes. The feature label is supplied with
`RSJSON_FEATURES` when running the matrix because Cargo does not expose
dependency features as crate `cfg` values. The raw outputs were captured under
`target/rsjson002/` and removed as reproducible build artifacts after their
results were recorded here.

| Baseline item | Result |
| --- | --- |
| Revision | `af5342897dedfbd4b8451fdee50d346600b5837d` |
| T2 source scope | `Cargo.toml`, `benches/unique_decode_bench.rs`, `benches/unique_decode_support/mod.rs`, plus the two new lexical-array bench files; no production source changed |
| `Cargo.lock` SHA-256 | `65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a` |
| Toolchain | `rustc 1.94.0 (4a4ef493e 2026-03-02)`, Cargo 1.94.0, Criterion 0.8.2 |
| Host | Ubuntu 24.04.4 LTS, Linux 7.0.0-38-generic, Intel Core i5-9600K, 6 CPUs, `powersave`, allowed CPU affinity 0–5; 23 logged-in users |
| Allocation matrix | 8 feature configurations × 3 successful runs; each run contains 76 non-JCS rows or 100 JCS rows. Full per-run values were captured in `target/rsjson002/allocation-summary.csv` (SHA-256 `7586cf83f6936eec58c36e54502bea20efb97190816b4d124c8b3f554f730452`); the generated file was removed after the results were recorded here. |
| Full Criterion samples | Not collected: shared-host load remained 28.86–45.75 on a 6-CPU host with concurrent Rust, Python, and Miri work. Criterion samples would not provide a dependable latency baseline under these conditions. |

The allocation values below are the unlimited 4096-scalar fixture. Each value
is identical across the three repeats; `requested` is cumulative requested
bytes, and `end delta` is recorded with the result alive. The complete summary
also contains finite-budget rows, 512-item arrays, records, all chain depths,
and each applicable path.

| Features | Path | Allocations | Requested | Peak extra | End delta |
| --- | --- | ---: | ---: | ---: | ---: |
| `default` | `decode_value` | 15 | 393,536 | 262,592 | 131,072 |
| `arbitrary_precision` | `decode_value` | 4,111 | 408,810 | 277,866 | 146,346 |
| `preserve_order` | `decode_value` | 15 | 884,896 | 590,272 | 294,912 |
| both serde_json features | `decode_value` | 4,111 | 900,170 | 605,546 | 310,186 |
| `jcs` | `JcsDecoder` | 15 | 393,568 | 262,528 | 131,072 |
| `jcs` + `arbitrary_precision` | `JcsDecoder` | 4,111 | 417,034 | 285,994 | 154,538 |
| `jcs` + `preserve_order` | `JcsDecoder` | 15 | 884,928 | 590,208 | 294,912 |
| all three features | `JcsDecoder` | 4,111 | 908,394 | 613,674 | 318,378 |

The existing comparison target also completed once with exit code 0:
`cargo bench --locked --bench unique_decode_allocations`; its 783 output rows
were captured in `target/rsjson002/unique-decode-allocations-default.log` and
removed after review. It provides the existing typed/seed controls, not a
directly comparable estimate of removable lexical-array buffering.

The measurements show that `arbitrary_precision` adds one allocation per
integer in this fixture, while `preserve_order` materially changes array
requested and retained bytes. These are configuration-specific baselines, not
evidence of an optimization. For the 4096-deep chain, default `decode_value`
reported 1,572,416 requested bytes and a 131,072-byte end delta; with
`preserve_order`, the corresponding values were 2,391,616 and 294,912 bytes.
The JCS path differed from default `decode_value` by 32 requested bytes on the
4096-scalar fixture. JCS number representation is binary64, so its preflight
checks exact shape and values using floating-point numeric comparison.

Load was checked repeatedly during the run: at local 23:44, 23:57, 23:58, and
23:59 on 2026-10-30, 1-minute load was 45.75, 37.34, 36.11, and 28.86; 5- and
15-minute averages also stayed above 32 at the last reading. A further
post-midnight reading on 2026-10-31 was 32.39/32.81/33.01. Other worktrees
had active Rust compilers, Python jobs, and Miri. Therefore full Criterion
sampling is deferred; no latency conclusion is drawn. The incomplete first
JCS attempt (`alloc-jcs-r1.log`) failed its initial assertion because it
compared JCS `0.0` to serde_json `0`; it is retained as a harness failure and
excluded. The corrected binary64-aware preflight passed for all JCS matrix
runs.

The allocation matrix runs each row three times and captures complete
stdout/stderr in `target/rsjson002/` for review. These generated outputs are
removed after the results are recorded; rerun the commands to regenerate them:

```bash
RSJSON_FEATURES=default cargo bench --locked --bench lexical_array_allocations
RSJSON_FEATURES=arbitrary_precision cargo bench --locked --features serde_json/arbitrary_precision --bench lexical_array_allocations
RSJSON_FEATURES=preserve_order cargo bench --locked --features serde_json/preserve_order --bench lexical_array_allocations
RSJSON_FEATURES=arbitrary_precision+preserve_order cargo bench --locked --features "serde_json/arbitrary_precision serde_json/preserve_order" --bench lexical_array_allocations
RSJSON_FEATURES=jcs cargo bench --locked --features jcs --bench lexical_array_allocations
RSJSON_FEATURES=jcs+arbitrary_precision cargo bench --locked --features "jcs serde_json/arbitrary_precision" --bench lexical_array_allocations
RSJSON_FEATURES=jcs+preserve_order cargo bench --locked --features "jcs serde_json/preserve_order" --bench lexical_array_allocations
RSJSON_FEATURES=jcs+arbitrary_precision+preserve_order cargo bench --locked --features "jcs serde_json/arbitrary_precision serde_json/preserve_order" --bench lexical_array_allocations
cargo bench --locked --bench unique_decode_bench -- lexical_arrays --save-baseline rsjson002-before
cargo bench --locked --bench unique_decode_allocations
```

Any later latency comparison should use Criterion's complete default sampling.
Compare repeated runs made on the same host and toolchain; retain the full
Criterion output and treat noisy or overlapping intervals as inconclusive.

### RSJSON-002 candidate comparison and decision

An experimental candidate stored each open lexical array directly in
`SafeValueBuilder`. It passed the focused owner-unwind, validated-decode, JCS,
and decoder-session tests. Allocation measurements were repeated three times
for every feature configuration; values below were identical in all repeats.
The candidate reduced requested bytes and `peak_extra` for the 512- and
4096-scalar arrays while leaving `end_delta` unchanged:

| Features / path | Items | Requested bytes, baseline → candidate | `peak_extra`, baseline → candidate | `end_delta`, baseline = candidate |
| --- | ---: | ---: | ---: | ---: |
| `default` / `decode_value` | 512 | 49,472 → 33,088 | 33,216 → 16,832 | 16,384 |
| `default` / `decode_value` | 4096 | 393,536 → 262,464 | 262,592 → 131,520 | 131,072 |
| `arbitrary_precision` / `decode_value` | 512 | 50,898 → 34,514 | 34,642 → 18,258 | 17,810 |
| `arbitrary_precision` / `decode_value` | 4096 | 408,810 → 277,738 | 277,866 → 146,794 | 146,346 |
| `preserve_order` / `decode_value` | 512 | 110,752 → 73,920 | 74,176 → 37,344 | 36,864 |
| `preserve_order` / `decode_value` | 4096 | 884,896 → 590,016 | 590,272 → 295,392 | 294,912 |
| both serde_json features / `decode_value` | 512 | 112,178 → 75,346 | 75,602 → 38,770 | 38,290 |
| both serde_json features / `decode_value` | 4096 | 900,170 → 605,290 | 605,546 → 310,666 | 310,186 |
| `jcs` / `JcsDecoder` | 512 | 49,504 → 33,120 | 33,152 → 16,768 | 16,384 |
| `jcs` / `JcsDecoder` | 4096 | 393,568 → 262,496 | 262,528 → 131,456 | 131,072 |
| `jcs` + `arbitrary_precision` / `JcsDecoder` | 512 | 51,954 → 35,570 | 35,602 → 19,218 | 18,834 |
| `jcs` + `arbitrary_precision` / `JcsDecoder` | 4096 | 417,034 → 285,962 | 285,994 → 154,922 | 154,538 |
| `jcs` + `preserve_order` / `JcsDecoder` | 512 | 110,784 → 73,952 | 74,112 → 37,280 | 36,864 |
| `jcs` + `preserve_order` / `JcsDecoder` | 4096 | 884,928 → 590,048 | 590,208 → 295,328 | 294,912 |
| all three features / `JcsDecoder` | 512 | 113,234 → 76,402 | 76,562 → 39,730 | 39,314 |
| all three features / `JcsDecoder` | 4096 | 908,394 → 613,514 | 613,674 → 318,794 | 318,378 |

The candidate met the allocation-reduction and retained-capacity checks. The
latency gate was not measured: repeated host readings showed sustained load
far above the six available CPUs, with unrelated compilation, Python, and Miri
work active. Therefore the required latency comparison is unresolved, not a
pass or a measured regression. Under the design's keep gate, the production
candidate was withdrawn and the original array-frame implementation remains.
The experimental production diff and its candidate-only fault-injection
change were removed after the comparison. The allocation harness and this
measurement record remain; no performance conclusion is inferred from the
allocation reduction alone.

### 2026-10-31 review reassessment

The current review did not start a new baseline or candidate run. Before
sampling, the host reported 1/5/15-minute loads of `23.99/26.59/27.31` on six
online CPUs, the `powersave` governor, about 21 GiB available memory, and about
12 GiB of swap in use. Unrelated Rust and C++ compilation and Miri processes
were active across other repositories. The checkout was `116b170`
(`dev-starfish` starting revision), `Cargo.lock` SHA-256 was
`65c8972b1699dab45b55ef05f55906c87ffcb2a5547effe34fe1c5046ec2c39a`, and the
toolchain was Rust/Cargo 1.94.0. These conditions do not provide an idle,
comparable baseline; no allocation preflight or Criterion latency sample was
run, and no candidate was created. The array-frame production implementation
is unchanged. Reassess when no unrelated build or Miri work is active and
system load is stable below the six-CPU capacity for repeated baseline and
candidate runs. Status: unresolved pending a controlled measurement window.
