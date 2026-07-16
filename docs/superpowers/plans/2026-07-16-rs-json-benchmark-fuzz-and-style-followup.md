# rs-json Benchmark, Fuzz, and Style Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Measure and conditionally improve dense control-character normalization, clarify JSONC fence semantics, make fuzzing reproducible, and align every Rust item under `src/` with repository style rules.

**Architecture:** Extend the public `decode_value` Criterion path before touching production code, record a baseline, then compare an exact-capacity trial against explicit retain/reject thresholds. Keep documentation, fuzz configuration/corpus, and source-style corrections behavior-neutral and preserve all public paths and signatures.

**Tech Stack:** Rust 1.94 / edition 2024, serde, serde_json, Criterion 0.5, cargo-fuzz 0.13.2, libFuzzer.

## Global Constraints

- Evaluate only `rs-json`; do not inspect or modify downstream crates.
- Preserve all public types, signatures, defaults, errors, normalization rules, and parsing semantics.
- Do not add JSONC parsing, prose extraction, streaming, NDJSON, runtime dependencies, or public entry points.
- Do not move, delete, or add production source files without separate user approval.
- Use `apply_patch` for file edits; preserve unrelated user changes.
- Do not run `git add`, `git commit`, `git push`, or destructive Git commands.
- Run repository validation in this order: `./align-ci.sh`, then `./ci-check.sh`, then `./coverage.sh json` only when CI reports coverage below threshold.

---

### Task 1: Add scaling benchmarks and record the current baseline

**Files:**
- Modify: `benches/decoder_bench.rs`

**Interfaces:**
- Consumes: `LenientJsonDecoder::decode_value(&self, &str)` and the existing Criterion benchmark target.
- Produces: stable `control-characters/{plain,sparse,dense}/{1024,65536}` benchmark identifiers used for before/after comparison.

- [ ] **Step 1: Add the benchmark before changing production code**

Extend the Criterion imports with `BenchmarkId` and `Throughput`, then add these functions and register the new benchmark:

```rust
/// Runs scaling benchmarks for control-character normalization.
fn benchmark_control_character_scaling(c: &mut Criterion) {
    let decoder = LenientJsonDecoder::default();
    let mut group = c.benchmark_group("control-characters");

    for payload_bytes in [1_024_usize, 65_536] {
        for (name, control_stride) in [
            ("plain", None),
            ("sparse", Some(1_024)),
            ("dense", Some(2)),
        ] {
            let input = control_character_input(payload_bytes, control_stride);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(name, payload_bytes),
                &input,
                |bencher, input| {
                    bencher.iter(|| {
                        black_box(
                            decoder
                                .decode_value(black_box(input.as_str()))
                                .expect("benchmark input must decode as a value"),
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

/// Builds a JSON object whose string payload has the requested control density.
fn control_character_input(
    payload_bytes: usize,
    control_stride: Option<usize>,
) -> String {
    let mut input = String::with_capacity(payload_bytes + 11);
    input.push_str("{\"text\":\"");
    for index in 0..payload_bytes {
        if control_stride.is_some_and(|stride| index % stride == 0) {
            input.push('\u{0000}');
        } else {
            input.push('a');
        }
    }
    input.push_str("\"}");
    input
}
```

Change the registration to:

```rust
criterion_group!(
    benches,
    benchmark_decoder,
    benchmark_control_character_scaling
);
```

- [ ] **Step 2: Compile the benchmark target**

Run: `cargo bench --bench decoder_bench --no-run`

Expected: exit 0 and a compiled `decoder_bench` executable.

- [ ] **Step 3: Record the unchanged implementation baseline**

Run: `cargo bench --bench decoder_bench -- control-characters`

Expected: all six benchmark IDs complete. Save the Criterion estimates from the command output in the working notes; do not commit `target/criterion` artifacts.

- [ ] **Step 4: Review the task diff**

Run: `git --no-pager diff -- benches/decoder_bench.rs`

Expected: only imports, the two documented benchmark helpers, and Criterion registration change.

### Task 2: Trial exact control-character capacity and make the measured choice

**Files:**
- Modify conditionally: `src/internal/control_character_escaper.rs`
- Benchmark: `benches/decoder_bench.rs`

**Interfaces:**
- Consumes: the stable benchmark IDs from Task 1 and the existing `ControlCharacterEscaper::replacement` state transition.
- Produces conditionally: `escaped_capacity(input: &str, remainder_start: usize, replacement: &str, in_string: bool, in_escape: bool) -> usize`; otherwise retains the current implementation with corrected allocation documentation.

- [ ] **Step 1: Apply the exact-capacity trial**

At the first replacement, replace `String::with_capacity(input.len() + 5)` with:

```rust
let capacity = Self::escaped_capacity(
    input,
    index + ch.len_utf8(),
    replacement,
    in_string,
    in_escape,
);
let mut result = String::with_capacity(capacity);
```

Add this private helper after `replacement` and before `escaped_control_char`:

```rust
/// Calculates the exact output capacity after the first replacement.
///
/// # Arguments
///
/// * `input` - Complete input whose final capacity is calculated.
/// * `remainder_start` - Byte index immediately after the first replacement.
/// * `replacement` - Escape emitted for the first replaced character.
/// * `in_string` - JSON-string state after the first replacement.
/// * `in_escape` - Backslash state after the first replacement.
///
/// # Returns
///
/// The exact byte capacity required by the escaped output.
#[must_use]
fn escaped_capacity(
    input: &str,
    remainder_start: usize,
    replacement: &str,
    mut in_string: bool,
    mut in_escape: bool,
) -> usize {
    let mut additional_bytes = replacement.len() - 1;
    for ch in input[remainder_start..].chars() {
        if let Some(replacement) =
            Self::replacement(ch, &mut in_string, &mut in_escape)
        {
            additional_bytes += replacement.len() - ch.len_utf8();
        }
    }
    input.len() + additional_bytes
}
```

Update the type documentation for the trial to state that the no-replacement path borrows and the replacement path calculates exact capacity before creating its output string.

- [ ] **Step 2: Run focused behavior tests before measuring**

Run: `cargo test --test tests internal::lenient_json_normalizer_tests`

Expected: all normalizer tests pass with identical decoding behavior.

- [ ] **Step 3: Run the comparable Criterion measurement**

Run: `cargo bench --bench decoder_bench -- control-characters`

Expected: Criterion prints change estimates against Task 1's baseline for the same six IDs.

- [ ] **Step 4: Apply the pre-agreed decision rule**

Retain the trial only if both dense cases improve by at least 5% with statistically significant change and neither 64 KiB plain nor sparse case regresses by more than 3% with statistically significant change.

If retained, keep `escaped_capacity` and document the exact-capacity second scan. If rejected, remove the helper and restore the original one-pass implementation, but replace the inaccurate type text with:

```rust
/// Escapes raw ASCII control characters occurring within JSON strings.
///
/// The scanner borrows its input unless it finds a replacement. On the first
/// replacement it lazily creates an output [`String`], copies the already
/// scanned prefix, and appends all remaining transformed characters.
```

- [ ] **Step 5: Re-run the focused tests on the selected implementation**

Run: `cargo test --test tests internal::lenient_json_normalizer_tests`

Expected: all focused tests pass after either decision branch.

### Task 3: Clarify JSONC semantics and make scheduled fuzzing reproducible

**Files:**
- Modify: `README.md`
- Modify: `README.zh_CN.md`
- Modify: `src/markdown_fence_policy.rs`
- Modify: `src/json_decode_options.rs`
- Modify: `src/internal/lenient_json_normalizer.rs`
- Modify: `.github/workflows/fuzz.yml`
- Modify: `fuzz/.gitignore`
- Create: `fuzz/corpus/decoder/plain-json`
- Create: `fuzz/corpus/decoder/backtick-fence`
- Create: `fuzz/corpus/decoder/tilde-fence`
- Create: `fuzz/corpus/decoder/mixed-line-endings`
- Create: `fuzz/corpus/decoder/raw-control-character`
- Create: `fuzz/corpus/decoder/unmatched-backslash`
- Create: `fuzz/corpus/decoder/jsonc-label-standard-json`

**Interfaces:**
- Consumes: the existing JSON-only fence policy and `fuzz/fuzz_targets/decoder.rs` target.
- Produces: explicit label-only JSONC documentation, pinned cargo-fuzz installation, and deterministic initial corpus inputs.

- [ ] **Step 1: Clarify the Rustdoc contract**

Append this semantic sentence to `MarkdownFencePolicy::JsonOnly`, `JsonDecodeOptions::json_code_fences_only`, and `is_json_code_fence_tag` documentation, adapting grammar to each item:

```rust
/// The `jsonc` token is accepted only as a fence label; fenced content must
/// still be standard JSON without comments or trailing commas.
```

- [ ] **Step 2: Clarify both README files**

After the JSON-only fence option description in `README.md`, add:

```markdown
`jsonc` is accepted only as a Markdown fence label. The fenced content is
still parsed as standard JSON, so comments and trailing commas remain invalid.
```

Add the matching paragraph to `README.zh_CN.md`:

```markdown
`jsonc` 仅作为 Markdown 代码块标签被识别。代码块内容仍按标准 JSON
解析，因此注释和尾随逗号依然无效。
```

- [ ] **Step 3: Pin cargo-fuzz**

Change the workflow installation command to:

```yaml
run: cargo install cargo-fuzz --version 0.13.2 --locked
```

- [ ] **Step 4: Add the initial corpus**

Create the corpus files with these exact logical contents; `mixed-line-endings` uses literal CR/LF bytes as annotated, not backslash characters:

```text
plain-json:                 {"ok":true}
backtick-fence:             ```json LF {"ok":true} LF ```
tilde-fence:                ~~~json LF {"ok":true} LF ~~~
mixed-line-endings:         ```json LF {"ok":true} CR ``` LF
raw-control-character:      {"text":"line one LF line two"}
unmatched-backslash:        {"text":"unterminated\
jsonc-label-standard-json:  ```jsonc LF {"ok":true} LF ```
```

- [ ] **Step 5: Verify the documented behavior and fuzz target build**

Run:

```bash
cargo test --test tests internal::lenient_json_normalizer_tests
cargo +nightly-2026-06-05 fuzz build decoder
```

Expected: JSON-only fence tests pass and the decoder fuzz target builds with cargo-fuzz 0.13.2.

### Task 4: Correct the complete `src/` Rust style inventory

**Files:**
- Modify: `src/internal/control_character_escaper.rs`
- Modify: `src/internal/lenient_json_normalizer.rs`
- Modify: `src/json_decode_error.rs`
- Modify: `src/json_decode_options.rs`
- Modify: `src/json_top_level_kind.rs`
- Modify: `src/lenient_json_decoder.rs`
- Audit without expected edits: every other `src/**/*.rs` file

**Interfaces:**
- Consumes: repository Rustdoc, method-order, inline, must-use, organization, and complexity rules.
- Produces: behavior-neutral attributes/docs with no public signature or path changes.

- [ ] **Step 1: Reconfirm organization, docs, and method order**

Inventory every type and function under `src/`. Confirm one type per file, complete copyright headers, explicit imports, private helper placement, external test mapping, inherent constructor/visibility order, and applicable Rustdoc headings. Do not move files: the current organization and method order already satisfy the rules.

- [ ] **Step 2: Correct inline classification**

Apply these exact attribute changes:

```text
src/internal/control_character_escaper.rs
- escaped_control_char: remove #[inline] because the 33-arm match is branch-heavy.

src/internal/lenient_json_normalizer.rs
- trim_if_enabled: change #[inline(always)] to #[inline].
- strip_utf8_bom: change #[inline(always)] to #[inline].
- is_json_code_fence_tag: retain #[inline].
- same_marker_fence_len: retain #[inline].
```

Retain `inline(always)` on getters, builders, defaults, and pure forwarding methods; retain `inline` on short constructors/classifiers with little branching; add no inline attribute to loops or complex parsing methods.

- [ ] **Step 3: Correct must-use semantics**

Apply this complete semantic inventory:

```text
Add #[must_use]:
- JsonDecodeError::{input_too_large, empty_input, invalid_json,
  unexpected_top_level, deserialize, from_serde_error, redacted_message}
- JsonTopLevelKind::of_normalized_json
- LenientJsonDecoder::map_decode_error
- LenientJsonNormalizer::{trim_if_enabled, trim_cow_if_enabled,
  strip_utf8_bom, strip_markdown_code_fence, is_json_code_fence_tag}
- ControlCharacterEscaper::escaped_capacity, only if Task 2 retains it

Remove redundant #[must_use] because Option carries the warning contract:
- JsonDecodeError::{expected_top_level, actual_top_level,
  normalized_input_bytes, max_input_bytes, normalized_line,
  normalized_column}
- JsonDecodeOptions::max_input_bytes
```

Retain existing attributes on constructors, getters returning unprotected
primitive/reference/domain values, builders, and pure transforms. Do not add
attributes to methods returning `Result` or other already protected results.

- [ ] **Step 4: Complete the remaining Rustdoc gap**

Document `LenientJsonDecoder::normalizer` as:

```rust
/// Stores the configured normalization pipeline.
normalizer: LenientJsonNormalizer,
```

Recheck that the selected Task 2 allocation documentation matches the retained implementation and that every changed function keeps all applicable `# Arguments`, `# Returns`, and `# Errors` sections.

- [ ] **Step 5: Compile docs and run Clippy-sensitive checks**

Run:

```bash
cargo doc --no-deps
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: both commands exit 0 with no missing-doc, attribute, or lint warning.

### Task 5: Run prescribed validation and final source re-audit

**Files:**
- Review: all changed files
- Review: every file under `src/`

**Interfaces:**
- Consumes: the selected benchmark implementation and all documentation, fuzz, corpus, and style changes.
- Produces: fresh verification evidence and a final correction report.

- [ ] **Step 1: Run repository alignment**

Run: `./align-ci.sh`

Expected: exit 0. Immediately inspect any script-generated edits with `git --no-pager diff` and keep only in-scope alignment changes.

- [ ] **Step 2: Run CI-equivalent checks**

Run: `./ci-check.sh`

Expected: exit 0 with tests, formatting, lint, documentation, and configured coverage checks passing.

- [ ] **Step 3: Run conditional coverage only when required**

If and only if Step 2 reports coverage below threshold, run: `./coverage.sh json`

Expected: JSON coverage output identifies exact uncovered files/lines; add only meaningful in-scope tests, then repeat Steps 1 and 2.

- [ ] **Step 4: Run final specialized checks**

Run:

```bash
cargo bench --bench decoder_bench --no-run
cargo +nightly-2026-06-05 fuzz build decoder
git diff --check
```

Expected: benchmark and fuzz targets build, and `git diff --check` reports no whitespace errors.

- [ ] **Step 5: Re-audit and report**

Re-read every `src/**/*.rs` file and compare all functions against the inline/must-use inventory. Run `git status --short`, `git --no-pager diff --stat`, and `git --no-pager diff`; report the benchmark decision and measured changes, corrected style categories, exact validation results, unresolved issues, and unchecked scope. Do not commit or push.
