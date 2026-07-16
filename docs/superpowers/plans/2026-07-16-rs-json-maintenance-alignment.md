# rs-json Maintenance Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align `qubit-json` documentation, Rust layout/style, external tests, design documents, and continuous fuzzing without changing public API or decoder behavior.

**Architecture:** Keep the normalization and decoding data flow untouched. Apply documentation, ordering, and attributes within existing type files; move black-box normalization tests into a mirrored `tests/internal` module; add an isolated scheduled fuzz workflow; then synchronize the Chinese design inventories.

**Tech Stack:** Rust 2024, Serde, serde_json, cargo-fuzz/libFuzzer, GitHub Actions, repository `rs-ci` scripts.

## Global Constraints

- Preserve every public type, method signature, default option, error kind, error stage, normalization rule, and privacy behavior in `0.4.0`.
- Add no runtime dependency and no JSON repair capability.
- Keep private implementation private; tests observe behavior only through `qubit_json` public APIs.
- Use the canonical copyright header in every Rust file.
- Run `./align-ci.sh` before `./ci-check.sh`; run `./coverage.sh json` only if CI reports coverage below threshold.
- Do not run `git add`, `git commit`, or `git push` unless the user explicitly authorizes it.

---

### Task 1: Align Rustdoc, method order, and inline attributes

**Files:**
- Modify: `src/json_decode_options.rs`
- Modify: `src/lenient_json_decoder.rs`
- Modify: `src/json_decode_error.rs`
- Modify: `src/json_decode_error_kind.rs`
- Modify: `src/json_decode_stage.rs`
- Modify: `src/json_top_level_kind.rs`
- Modify: `src/internal/lenient_json_normalizer.rs`
- Modify: `src/internal/control_character_escaper.rs`

**Interfaces:**
- Consumes: Existing public and private signatures at `HEAD`.
- Produces: The same signatures and behavior, with complete Rustdoc, rule-compliant method order, and body-shape-based inline attributes.

- [ ] **Step 1: Capture the public signature baseline**

Run:

```bash
rg -n '^\s*(pub|pub\(crate\))?\s*(const\s+)?fn |^\s*pub enum |^\s*pub struct ' src
```

Expected: inventory includes the four decoder entry points, all option getters/builders, error accessors, and private helpers. Save the output for comparison; do not edit signatures.

- [ ] **Step 2: Reorder `JsonDecodeOptions` methods and add documentation/attributes**

Keep the constructor group first:

```rust
pub const fn lenient() -> Self;
pub const fn strict() -> Self;
pub const fn json_code_fences_only() -> Self;
```

Then arrange related pairs exactly in this order:

```rust
pub const fn trim_whitespace(&self) -> bool;
pub const fn with_trim_whitespace(self, enabled: bool) -> Self;
pub const fn strip_utf8_bom(&self) -> bool;
pub const fn with_strip_utf8_bom(self, enabled: bool) -> Self;
pub const fn markdown_fence_policy(&self) -> MarkdownFencePolicy;
pub const fn with_markdown_fence_policy(self, policy: MarkdownFencePolicy) -> Self;
pub const fn escape_control_chars_in_strings(&self) -> bool;
pub const fn with_escape_control_chars_in_strings(self, enabled: bool) -> Self;
pub const fn max_input_bytes(&self) -> Option<usize>;
pub const fn with_max_input_bytes(self, limit: Option<usize>) -> Self;
pub const fn error_privacy_policy(&self) -> ErrorPrivacyPolicy;
pub const fn with_error_privacy_policy(self, policy: ErrorPrivacyPolicy) -> Self;
```

For each getter/builder, add `#[inline(always)]`. Constructors use `#[inline]`. Add structured Rustdoc using this exact shape, adapted to each field:

```rust
/// Returns whether surrounding whitespace is removed.
///
/// # Returns
///
/// `true` when trimming is enabled; otherwise, `false`.
#[inline(always)]
#[must_use]
pub const fn trim_whitespace(&self) -> bool { ... }

/// Returns a copy with surrounding-whitespace trimming configured.
///
/// # Arguments
///
/// * `enabled` - Whether to enable trimming.
///
/// # Returns
///
/// The updated option set.
#[inline(always)]
#[must_use]
pub const fn with_trim_whitespace(mut self, enabled: bool) -> Self { ... }
```

Document `Option<usize>` with explicit `Some(limit)` and `None` semantics. Document `Default::default` as returning the lenient preset.

- [ ] **Step 3: Complete and classify decoder methods**

Use `#[inline(always)]` on `new`, `options`, `decode_object`, and `decode_array`. Do not inline `decode`, `decode_value`, `decode_with_top_level`, parsing, validation, deserialization, or error classification helpers.

For each public decode method, use the following complete contract, specialized by top-level shape:

```rust
/// Decodes `input` into the requested type.
///
/// # Arguments
///
/// * `input` - Raw JSON text to normalize and decode.
///
/// # Returns
///
/// The decoded value.
///
/// # Errors
///
/// Returns [`JsonDecodeError`] when normalization, syntax parsing,
/// top-level validation, or target deserialization fails as applicable.
```

Add private-helper Rustdoc naming all parameters and distinguishing `InvalidJson`, `UnexpectedTopLevel`, and `Deserialize`; preserve method bodies byte-for-byte apart from moved attributes/comments.

- [ ] **Step 4: Reorder and document `JsonDecodeError`**

Order inherent methods as:

1. Restricted constructors: `input_too_large`, `empty_input`, `invalid_json`, `unexpected_top_level`, `deserialize`.
2. Private constructor: `from_serde_error`.
3. Public accessors in their current logical order.
4. Private non-constructor helper: `redacted_message`.

Remove inline attributes from `input_too_large`, `empty_input`, and `unexpected_top_level`. Use `#[inline(always)]` for `invalid_json`, `deserialize`, and all accessors. `from_serde_error` and `redacted_message` remain without inline attributes.

Every constructor/helper gets `# Arguments` and `# Returns`. Every `Option` accessor explicitly documents both states, for example:

```rust
/// # Returns
///
/// `Some(kind)` when constrained decoding rejected a valid top-level value;
/// otherwise, `None`.
```

Document `PartialEq::eq`, `Display::fmt`, and `Error::source`; `fmt` must state that formatter errors are propagated.

- [ ] **Step 5: Complete enum and internal-helper Rustdoc**

Apply the same structured headings to:

- `JsonDecodeErrorKind::{fmt, from_str}`;
- `JsonDecodeStage::fmt`;
- `JsonTopLevelKind::{of, of_normalized_json, from, fmt, from_str}`;
- every method on `LenientJsonNormalizer`;
- every method on `ControlCharacterEscaper`.

Use `#[inline(always)]` only on pure forwarders/accessors, retain `#[inline]` on short classification helpers, and leave loops/branch-heavy functions unannotated. Do not claim panics or safety requirements that do not exist.

- [ ] **Step 6: Verify signatures and behavior-bearing lines did not change**

Run:

```bash
git --no-pager diff --word-diff=plain -- src
```

Expected: only documentation, method movement, and attributes differ; all expressions and signatures remain unchanged.

Run:

```bash
cargo test --test tests
```

Expected: all existing integration tests pass.

---

### Task 2: Mirror the internal normalization test path

**Files:**
- Create: `tests/internal/mod.rs`
- Move: `tests/lenient_json_normalizer_tests.rs` → `tests/internal/lenient_json_normalizer_tests.rs`
- Modify: `tests/mod.rs`

**Interfaces:**
- Consumes: Existing black-box tests using `LenientJsonDecoder`.
- Produces: The same tests loaded through `tests::internal::lenient_json_normalizer_tests`.

- [ ] **Step 1: Move the test file without changing its contents**

Create the destination using `apply_patch`, copying the complete existing file, then delete the old path using `apply_patch`. Do not use `mv` so the content change remains reviewable under the repository editing rules.

- [ ] **Step 2: Add the nested test module entry**

Create `tests/internal/mod.rs` with the canonical header and:

```rust
//! Tests for private normalization behavior observed through public APIs.

mod lenient_json_normalizer_tests;
```

Replace this line in `tests/mod.rs`:

```rust
mod lenient_json_normalizer_tests;
```

with:

```rust
mod internal;
```

- [ ] **Step 3: Run the moved tests**

Run:

```bash
cargo test --test tests internal::lenient_json_normalizer_tests
```

Expected: every moved normalization test passes and the old root module path is absent.

---

### Task 3: Expand and schedule bounded fuzzing

**Files:**
- Modify: `fuzz/fuzz_targets/decoder.rs`
- Create: `.github/workflows/fuzz.yml`
- Modify: `README.md`
- Modify: `README.zh_CN.md`

**Interfaces:**
- Consumes: `JsonDecodeOptions`, `MarkdownFenceClosing`, `MarkdownFencePolicy`, and the four public decoder methods.
- Produces: A four-policy no-panic fuzz matrix and an independent scheduled workflow.

- [ ] **Step 1: Expand the decoder matrix**

Add the canonical header and imports:

```rust
use qubit_json::{
    JsonDecodeOptions,
    LenientJsonDecoder,
    MarkdownFenceClosing,
    MarkdownFencePolicy,
};
```

Use this decoder matrix:

```rust
let decoders = [
    LenientJsonDecoder::default(),
    LenientJsonDecoder::new(JsonDecodeOptions::strict()),
    LenientJsonDecoder::new(JsonDecodeOptions::json_code_fences_only()),
    LenientJsonDecoder::new(
        JsonDecodeOptions::json_code_fences_only()
            .with_markdown_fence_policy(MarkdownFencePolicy::JsonOnly {
                closing: MarkdownFenceClosing::Required,
            }),
    ),
];
```

Retain all four decode calls and UTF-8 filtering.

- [ ] **Step 2: Add the independent workflow**

Create `.github/workflows/fuzz.yml` with:

```yaml
name: Rust Fuzz

on:
  schedule:
    - cron: "23 18 * * *"
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: ${{ github.repository }}-${{ github.workflow }}
  cancel-in-progress: true

jobs:
  decoder:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout source
        uses: actions/checkout@v6
        with:
          fetch-depth: 1
          submodules: recursive
      - name: Install Rust nightly
        run: rustup toolchain install nightly-2026-06-05 --profile minimal
      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz --locked
      - name: Fuzz decoder
        working-directory: fuzz
        run: cargo +nightly-2026-06-05 fuzz run decoder -- -max_total_time=300
```

The daily run is bounded to five minutes and never runs on pull requests.

- [ ] **Step 3: Align fuzz documentation**

Update both READMEs to state that the decoder target covers default, strict,
JSON-only, and required-closing policies and is run by the scheduled workflow.
Keep existing local build/run commands.

- [ ] **Step 4: Build the fuzz target locally**

Run:

```bash
(cd fuzz && cargo +nightly-2026-06-05 fuzz build decoder)
```

Expected: the `decoder` fuzz target builds successfully.

---

### Task 4: Align PRD and design inventories

**Files:**
- Modify: `doc/json_design.zh_CN.md`
- Modify: `doc/json_prd.zh_CN.md`

**Interfaces:**
- Consumes: Final source, test, benchmark, fuzz, and workflow layout from Tasks 1–3.
- Produces: Documentation inventories matching the filesystem exactly.

- [ ] **Step 1: Update the design tree**

Add these omitted source entries:

```text
json_decode_stage.rs
markdown_fence_closing.rs
markdown_fence_policy.rs
```

Represent normalization tests as:

```text
tests/
  internal/
    mod.rs
    lenient_json_normalizer_tests.rs
```

List the stage and Markdown policy test files, `benches/decoder_bench.rs`,
`fuzz/fuzz_targets/decoder.rs`, and `.github/workflows/fuzz.yml`.

- [ ] **Step 2: Update testing and validation prose**

State that normalization internals are tested through public decoder behavior,
and that scheduled fuzzing supplements deterministic integration tests. Do not
claim fuzzing runs on every PR.

- [ ] **Step 3: Update the PRD test inventory**

Make the PRD list include every public-type test and the nested internal
normalizer test path. Preserve all product requirements and non-goals.

- [ ] **Step 4: Compare documented paths with the filesystem**

Run:

```bash
rg --files src tests benches fuzz/fuzz_targets .github/workflows | sort
```

Expected: every relevant Rust/workflow path appears in the design inventory and no obsolete root `tests/lenient_json_normalizer_tests.rs` remains.

---

### Task 5: Run repository verification and final audit

**Files:**
- Inspect: all files changed in Tasks 1–4

**Interfaces:**
- Consumes: Completed maintenance alignment.
- Produces: Fresh formatting, CI, coverage, fuzz-build, and diff evidence.

- [ ] **Step 1: Run repository alignment**

Run:

```bash
./align-ci.sh
```

Expected: exit 0. Inspect any formatting edits before continuing.

- [ ] **Step 2: Re-audit moved methods and documentation**

Run:

```bash
rg -n '^\s*(pub|pub\(crate\))?\s*(const\s+)?fn |# Arguments|# Returns|# Errors|# Panics|# Safety|#\[inline' src
git --no-pager diff -- src tests fuzz .github doc README.md README.zh_CN.md
```

Expected: signatures match the baseline; method order, headings, attributes,
test paths, workflow, and inventories match the approved design.

- [ ] **Step 3: Run CI-equivalent checks**

Run:

```bash
./ci-check.sh
```

Expected: exit 0 with formatting, Clippy, style, build, tests, docs, package,
coverage, and audit checks passing.

- [ ] **Step 4: Conditionally inspect coverage**

Only if Step 3 reports coverage below threshold, run exactly:

```bash
./coverage.sh json
```

Expected: coverage report identifies any remaining gap; add no behavior-changing
test solely to satisfy a percentage.

- [ ] **Step 5: Confirm the final worktree and requirements**

Run:

```bash
git status --short
git --no-pager diff --check
git --no-pager diff --stat
```

Expected: only approved files are changed, no whitespace errors exist, and no
unrelated downstream or runtime code is modified. Do not stage or commit unless
the user separately authorizes it.
