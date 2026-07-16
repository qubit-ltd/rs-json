# rs-json Performance, Fuzz, and Style Alignment Implementation Plan

> **For the implementation agent:** Execute this plan in order using test-driven
> development. Keep all work in the linked `codex/rs-json-c-improvements`
> worktree. Do not create a Git commit unless separately authorized.

**Goal:** Preserve rs-json's public decoding contract while removing redundant
validation on successful typed object/array decodes, consolidating control
character normalization, and adding maintainable regression, benchmark, fuzz,
documentation, and Rust style coverage.

**Architecture:** Classify normalized JSON before typed object/array decoding.
For an expected-kind mismatch, parse as `RawValue` solely to preserve the
distinction between malformed JSON and a valid-but-unexpected top-level value;
for a matching kind, deserialize the target type directly. Move normalizer
supporting types into private, one-type-per-file internal modules and make the
control-character escaper allocate only after its first replacement. Exercise
the public surface through integration tests, Criterion benchmarks, and a
standalone cargo-fuzz package.

**Tech Stack:** Rust 1.94 / edition 2024, serde, serde_json, Criterion,
libfuzzer-sys, cargo-fuzz.

---

### Task 1: Add decoder regression coverage before changing the decoder

**Files:**
- Modify: `tests/lenient_json_decoder_tests.rs`
- Modify: `tests/mod.rs` only if a new focused integration test module is
  introduced

**Step 1: Write the failing tests**

Add external integration cases for `decode_object` and `decode_array` that
cover:

- valid typed JSON remains deserializable, including a `u128` value and a
  duplicate-key payload whose typed result follows serde's existing behaviour;
- a syntactically malformed payload whose first significant byte identifies the
  wrong top-level kind reports `InvalidJson`, not `UnexpectedTopLevel`;
- a syntactically valid wrong top-level payload reports `UnexpectedTopLevel`.

Use existing shared test fixtures where possible. Keep helper types in their
own private test-module files if new types are necessary.

**Step 2: Run the focused test command to verify the baseline contract**

Run: `cargo test --test mod lenient_json_decoder_tests`

Expected: the existing behaviour passes. These tests are semantic regression
guards for a performance-only internal change; their pre-change success is
expected.

### Task 2: Make typed object/array decoding avoid redundant successful parses

**Files:**
- Modify: `src/lenient_json_decoder.rs`
- Modify: `src/json_decode_error.rs` only when method reordering and docs need
  an adjacent change
- Test: `tests/lenient_json_decoder_tests.rs`

**Step 1: Make the smallest production change**

In `decode_with_top_level`, normalize once, determine the top-level kind, and
then:

1. when the kind matches, directly deserialize the target type so serde
   performs the one necessary parse;
2. when the kind differs, call the existing `validate_json` first, then return
   `UnexpectedTopLevel` only if validation succeeds.

Do not alter error privacy, input-size checks, presets, error kinds, or public
signatures. Retain existing direct-deserialization error classification.

**Step 2: Run focused regression tests**

Run: `cargo test --test mod lenient_json_decoder_tests`

Expected: all focused decoder tests pass.

**Step 3: Format**

Run: `cargo fmt --check`

Expected: clean formatting; run `cargo fmt` only if needed, then repeat the
check.

### Task 3: Consolidate raw-control-character normalization

**Files:**
- Add: `src/internal/mod.rs`
- Add: `src/internal/control_character_escaper.rs`
- Add: `src/internal/lenient_json_normalizer.rs`
- Add: `src/internal/markdown_fence.rs`
- Delete: `src/lenient_json_normalizer.rs`
- Modify: `src/lib.rs`
- Modify: `src/lenient_json_decoder.rs`
- Modify: `tests/lenient_json_normalizer_tests.rs`

**Step 1: Add regression tests**

Before moving implementation, add external cases proving that inputs without
C0 controls are unchanged and inputs with escaped/raw controls have the same
normalization output as the existing compatibility contract. Cover controls in
strings, outside strings, and escaped quote/backslash boundaries.

**Step 2: Run focused normalizer tests**

Run: `cargo test --test mod lenient_json_normalizer_tests`

Expected: baseline passes; test additions document preserved behaviour.

**Step 3: Refactor into internal one-type-per-file modules**

Move `LenientJsonNormalizer` and `MarkdownFence` to dedicated internal files.
Introduce `ControlCharacterEscaper` as the sole state machine. It must borrow
the original input until it sees the first raw C0 character that needs JSON
escaping, then allocate once and append the remaining transformed bytes.
Preserve quote, backslash, and Markdown-fence handling exactly.

**Step 4: Run focused tests and formatting**

Run:

```bash
cargo test --test mod lenient_json_normalizer_tests
cargo fmt --check
```

Expected: tests pass and formatting is clean.

### Task 4: Align documentation, rustdoc, module layout, and method order

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/json_decode_error.rs`
- Modify: `src/json_decode_options.rs`
- Modify: `src/json_top_level_kind.rs`
- Modify: `src/lenient_json_decoder.rs`
- Modify: `src/internal/*.rs`
- Modify: `README.md`
- Modify: `doc/json_prd.zh_CN.md`
- Modify: `doc/json_design.zh_CN.md`
- Modify: `Cargo.toml`

**Step 1: Add/align documentation**

Document all public items and their arguments, errors, and examples where
applicable. Add concise private rustdoc for private types, fields, and
non-trivial helper functions. Synchronize README/PRD/DESIGN with actual
defaults: strict-by-default options, no default maximum input size, lenient
preset behaviour, and UTF-8/C0 handling. Update Cargo's published-file include
list to match committed documentation and benchmark sources.

**Step 2: Apply style constraints**

Keep one type per source file; group constructors first, public methods next,
and private helpers last. Keep related getter/builder pairs adjacent. Apply
`#[inline]` only to short stateless helpers and thin forwards, and
`#[inline(always)]` only to appropriate trivial accessors/forwards. Do not add
unjustified attributes to complex parsing logic.

**Step 3: Verify public documentation**

Run: `cargo doc --no-deps`

Expected: documentation builds without warnings or broken links.

### Task 5: Add reproducible Criterion benchmarks

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Add: `benches/decoder_bench.rs`
- Modify: `README.md`
- Modify: `Cargo.toml` `include` list

**Step 1: Add the benchmark target first**

Create a Criterion benchmark for representative plain JSON, fenced JSON, and
raw-control-character JSON. Each case must separately measure `decode`,
`decode_object`, and `decode_array` where their expected top-level kind applies.

**Step 2: Add only the development dependency/configuration it requires**

Add Criterion under `[dev-dependencies]` and one `[[bench]]` entry with
`harness = false`. Regenerate the workspace's package lock through Cargo.

**Step 3: Compile and run a short benchmark smoke test**

Run: `cargo bench --bench decoder_bench --no-run`

Expected: benchmark compiles. If environment time permits, run the benchmark
with Criterion's short measurement options and record no performance claim
unless before/after data is comparably collected.

### Task 6: Add cargo-fuzz decoder coverage

**Files:**
- Add: `fuzz/Cargo.toml`
- Add: `fuzz/fuzz_targets/decoder.rs`
- Add: `fuzz/.gitignore`
- Modify: `README.md`
- Modify: `Cargo.toml` `include` list only if package publishing policy requires
  shipping fuzz-source metadata

**Step 1: Create a minimal public-surface fuzz target**

Create a cargo-fuzz package targeting the parent crate. Feed arbitrary UTF-8
input through every public decoder entry point and both strict/lenient options.
Discard decode results; the invariant is no panic. Construct small target types
locally within the fuzz target.

**Step 2: Build the fuzz target**

Run: `cargo fuzz build decoder` from `fuzz/`.

Expected: the target compiles. If `cargo-fuzz` is unavailable, run an
equivalent `cargo check --manifest-path fuzz/Cargo.toml` and state the tooling
limitation.

**Step 3: Document developer usage**

Add concise README commands for installing cargo-fuzz, building, and running
the target. State that fuzzing is development tooling, not a published runtime
dependency.

### Task 7: Run repository validation and independent review

**Files:**
- Review all changed files

**Step 1: Run required validation in project order**

Run:

```bash
./align-ci.sh
./ci-check.sh
```

Expected: both pass. Run coverage only if `ci-check.sh` reports the coverage
threshold is not met, following its reported command.

**Step 2: Run scoped confidence checks**

Run:

```bash
cargo test
cargo doc --no-deps
cargo bench --bench decoder_bench --no-run
```

Expected: all pass.

**Step 3: Review the diff and report results**

Inspect `git diff --check`, `git diff --stat`, and the changed public API/docs.
Perform a requirements-focused review for semantic regressions, test gaps,
documentation drift, and accidental public-surface changes. Resolve any
findings, rerun affected validation, and report commands/results without
creating a commit.
