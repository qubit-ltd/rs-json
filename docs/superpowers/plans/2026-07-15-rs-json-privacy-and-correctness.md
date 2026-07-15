# rs-json Privacy and Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task-by-task. Subagents are not authorized for this work.

**Goal:** Add a safe-by-default configurable error privacy policy, repair three confirmed normalization/diagnostic bugs, and replace the public-field Options API with an evolvable builder/getter API.

**Architecture:** `JsonDecodeOptions` remains the immutable configuration source and gains an `ErrorPrivacyPolicy`. Every error constructor receives the effective policy; redacted errors discard serde details at construction time, while detailed errors preserve current diagnostics. The existing normalizer pipeline and direct serde decoding design remain intact.

**Tech Stack:** Rust 1.94, edition 2024, serde, serde_json, integration tests under `tests/`, project `style-check.sh`, cargo-llvm-cov.

## Global Constraints

- The crate version changes from `0.3.6` to `0.4.0`; source compatibility is intentionally not preserved.
- `ErrorPrivacyPolicy::Redacted` is the default for every preset.
- Redacted `message`, `Display`, `Debug`, and standard `source` must contain no input value fragments.
- Detailed diagnostics preserve the current serde message and source.
- No new runtime dependency is allowed.
- Object/array double parsing and unrelated downstream issues remain out of scope.
- Do not create Git commits, add files to the index, or modify the OpenAI provider lock file.

---

### Task 1: Add the public privacy policy and Options contract

**Files:**
- Create: `src/error_privacy_policy.rs`
- Create: `tests/error_privacy_policy_tests.rs`
- Modify: `src/lib.rs`
- Modify: `src/json_decode_options.rs`
- Modify: `tests/mod.rs`
- Modify: `tests/lib_tests.rs`
- Modify: `tests/json_decode_options_tests.rs`

**Interfaces:**
- Produces: `ErrorPrivacyPolicy::{Redacted, Detailed}` and `Default<Output = Redacted>`.
- Produces: `JsonDecodeOptions::error_privacy_policy()` and `with_error_privacy_policy(...)`.
- Preserves: `JsonDecodeOptions: Copy + Eq`.

- [x] **Step 1: Write failing policy and Options tests**

Add an integration test module and assertions equivalent to:

```rust
use qubit_json::ErrorPrivacyPolicy;

#[test]
fn test_default_is_redacted() {
    assert_eq!(
        ErrorPrivacyPolicy::default(),
        ErrorPrivacyPolicy::Redacted,
    );
}
```

Extend Options tests so all presets assert
`options.error_privacy_policy() == ErrorPrivacyPolicy::Redacted`, and add a
builder assertion for `Detailed`. Extend the crate export smoke test to name the
new public type.

- [x] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test --test tests error_privacy_policy_tests
```

Expected: compilation fails because `ErrorPrivacyPolicy` and its module do not
exist.

- [x] **Step 3: Implement the minimal public policy**

Create the one-public-type source file:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ErrorPrivacyPolicy {
    #[default]
    Redacted,
    Detailed,
}
```

Document both variants, declare/re-export the module from `lib.rs`, add the
private Options field, initialize it in `lenient()` and `strict()`, and add the
getter/builder.

- [x] **Step 4: Run the focused tests and verify GREEN**

Run:

```bash
cargo test --test tests error_privacy_policy_tests
cargo test --test tests json_decode_options_tests
cargo test --test tests lib_tests
```

Expected: all selected tests pass.

---

### Task 2: Replace public Options fields with complete getters/builders

**Files:**
- Modify: `src/json_decode_options.rs`
- Modify: `src/lenient_json_normalizer.rs`
- Modify: `tests/json_decode_options_tests.rs`
- Modify: `tests/lenient_json_decoder_tests.rs`
- Modify: `tests/lenient_json_normalizer_tests.rs`
- Modify: `tests/json_decode_error_tests.rs`

**Interfaces:**
- Produces: getters `trim_whitespace`, `strip_utf8_bom`, `markdown_fence_policy`, `escape_control_chars_in_strings`, `max_input_bytes`, and `error_privacy_policy`.
- Produces: matching `with_*` builders; `with_max_input_bytes` accepts `Option<usize>`.
- Removes: external field access and struct literal construction.

- [x] **Step 1: Expand Options tests before changing production code**

Add assertions for every getter and builder, including the reversible size
limit:

```rust
let options = JsonDecodeOptions::strict()
    .with_trim_whitespace(true)
    .with_strip_utf8_bom(true)
    .with_markdown_fence_policy(policy)
    .with_escape_control_chars_in_strings(true)
    .with_max_input_bytes(Some(64));
assert_eq!(options.max_input_bytes(), Some(64));
assert_eq!(options.with_max_input_bytes(None).max_input_bytes(), None);
```

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test --test tests json_decode_options_tests
```

Expected: compilation fails on missing getters/builders and the new optional
size-limit signature.

- [x] **Step 3: Implement and migrate the Options API**

Make all fields private and implement the exact getters/builders from the
approved design. Replace normalizer field reads with getters. Replace every
test struct literal with a preset plus builders, for example:

```rust
JsonDecodeOptions::default()
    .with_trim_whitespace(false)
    .with_markdown_fence_policy(MarkdownFencePolicy::Disabled)
```

Replace size calls with `with_max_input_bytes(Some(limit))`.

- [x] **Step 4: Verify GREEN across the crate tests**

Run:

```bash
cargo test --all-targets --all-features
```

Expected: existing behavior tests and new Options tests pass.

---

### Task 3: Enforce redacted and detailed error behavior

**Files:**
- Modify: `src/json_decode_error.rs`
- Modify: `src/lenient_json_decoder.rs`
- Modify: `src/lenient_json_normalizer.rs`
- Modify: `tests/json_decode_error_tests.rs`

**Interfaces:**
- Produces: `JsonDecodeError::privacy_policy() -> ErrorPrivacyPolicy`.
- Redacted parse/deserialize errors: stable safe message, location retained, source discarded.
- Detailed parse/deserialize errors: current serde message and source retained.

- [x] **Step 1: Write failing privacy behavior tests**

Use a sentinel such as `TOP_SECRET` and assert all standard surfaces:

```rust
let error = LenientJsonDecoder::default()
    .decode::<u64>(r#""TOP_SECRET""#)
    .expect_err("type mismatch should fail");
assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
assert!(!error.message().contains("TOP_SECRET"));
assert!(!error.to_string().contains("TOP_SECRET"));
assert!(!format!("{error:?}").contains("TOP_SECRET"));
assert!(std::error::Error::source(&error).is_none());
```

Add the inverse test using `Detailed`, plus redacted/detailed invalid-JSON
source tests, location assertions, normalization-error policy assertions, and
an equality assertion showing policies differ.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test --test tests json_decode_error_tests
```

Expected: default errors still contain the sentinel and expose a source; the
privacy accessor is missing.

- [x] **Step 3: Implement privacy-aware error construction**

Add `privacy_policy` to `JsonDecodeError` and every constructor. In
`from_serde_error`, extract line/column first and then branch:

```rust
let (message, source) = match privacy_policy {
    ErrorPrivacyPolicy::Redacted => (
        Self::redacted_message(prefix, line, column),
        None,
    ),
    ErrorPrivacyPolicy::Detailed => (
        format!("{prefix}: {error}"),
        Some(Arc::new(error)),
    ),
};
```

Add a documented private helper that appends only normalized line/column to a
redacted prefix. Thread the policy from decoder/normalizer Options into parse,
deserialize, size, empty-input, and top-level error constructors. Include the
policy in `PartialEq`.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test --test tests json_decode_error_tests
cargo test --all-targets --all-features
```

Expected: all privacy and existing diagnostic tests pass.

---

### Task 4: Repair raw C0 controls after an unmatched backslash

**Files:**
- Modify: `tests/lenient_json_normalizer_tests.rs`
- Modify: `src/lenient_json_normalizer.rs`

**Interfaces:**
- Preserves: valid existing escapes and disabled-repair behavior.
- Changes: unmatched backslash plus raw C0 becomes one valid JSON escape.

- [x] **Step 1: Write the failing regression tests**

Loop over all `U+0000..=U+001F`, place each after one unmatched backslash in a
JSON string, decode, and assert that the resulting Rust string contains the
control character without an extra backslash. Add focused odd/even backslash
tests for LF and NUL.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test --test tests test_decode_value_escapes_control_char_after_unmatched_backslash
```

Expected: current implementation returns `InvalidJson`.

- [x] **Step 3: Implement the minimal state-machine fix**

In both the count and rewrite passes, recognize raw C0 before clearing
`in_escape`. For unmatched-backslash controls, write the escape string without
its first backslash; otherwise write the full escape. Keep non-C0 invalid
escapes unchanged.

- [x] **Step 4: Verify GREEN**

Run:

```bash
cargo test --test tests test_decode_value_escapes_control_char
cargo test --test tests lenient_json_normalizer_tests
```

Expected: new and existing control-character tests pass.

---

### Task 5: Repair mixed-line-ending closing fences

**Files:**
- Modify: `tests/lenient_json_normalizer_tests.rs`
- Modify: `src/lenient_json_normalizer.rs`

**Interfaces:**
- Changes only closing-line discovery; marker and closing-policy rules remain unchanged.

- [x] **Step 1: Write failing mixed-line-ending tests**

Add both directions:

```rust
"```json\n{\n\"a\":1\n}\r```"
"```json\r{\r\"a\":1\r}\n```"
```

Both must decode to `json!({"a": 1})`.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test --test tests test_decode_value_strips_code_fence_with_mixed_line_endings
```

Expected: at least the LF-body/CR-closing case reports trailing characters.

- [x] **Step 3: Implement the minimal closing-line fix**

Replace the fallback search with the greater of the final LF and CR indices:

```rust
let closing_line_start = trimmed_end
    .rfind('\n')
    .max(trimmed_end.rfind('\r'))
    .map_or(0, |index| index + 1);
```

- [x] **Step 4: Verify GREEN**

Run the focused tests and all normalizer tests. Expected: mixed and existing
fence cases pass.

---

### Task 6: Report normalized length for post-normalization emptiness

**Files:**
- Modify: `tests/lenient_json_normalizer_tests.rs`
- Modify: `src/json_decode_error.rs`
- Modify: `src/lenient_json_normalizer.rs`

**Interfaces:**
- Initial empty/whitespace failure: `normalized_input_bytes() == None`.
- Full pipeline resulting in empty text: `normalized_input_bytes() == Some(0)`.

- [x] **Step 1: Write failing diagnostic tests**

Extend existing empty, BOM-only, and empty-fence tests with exact
`normalized_input_bytes()` assertions.

- [x] **Step 2: Verify RED**

Run:

```bash
cargo test --test tests test_decode_value_reports_empty_input
```

Expected: BOM-only and empty fenced body return `None` instead of `Some(0)`.

- [x] **Step 3: Implement the constructor distinction**

Change `empty_input` to accept `Option<usize>`. Pass `None` from the initial
guard and `Some(input.len())` from the final pipeline check. Preserve raw input
bytes and the effective privacy policy.

- [x] **Step 4: Verify GREEN**

Run the focused empty-input tests and all crate tests. Expected: exact
diagnostics and all prior behavior pass.

---

### Task 7: Update version, docs, doctests, and direct downstream

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/lib.rs`
- Modify: `README.md`
- Modify: `README.zh_CN.md`
- Modify: `doc/json_prd.zh_CN.md`
- Modify: `doc/json_design.zh_CN.md`
- Modify: `/home/starfish/working/qubit/llmsdk/llmsdk-rust/rs-llmsdk-core/Cargo.toml`
- Modify: `/home/starfish/working/qubit/llmsdk/llmsdk-rust/rs-llmsdk-core/Cargo.lock`
- Modify: `/home/starfish/working/qubit/llmsdk/llmsdk-rust/rs-llmsdk-core/src/engine/entity_decoder.rs`
- Modify: `/home/starfish/working/qubit/llmsdk/llmsdk-rust/rs-llmsdk-core/tests/chat/entity_decoder_tests.rs`

**Interfaces:**
- Package and exact direct dependency version: `0.4.0`.
- `EntityDecoder::default()` keeps 1 MiB and now explicitly uses the redacted default.

- [x] **Step 1: Add a failing downstream privacy contract test**

Extend the core entity decoder tests to trigger a typed mismatch containing a
sentinel and assert the resulting `EngineError` display/debug strings omit it.
This proves the default policy survives the wrapper boundary.

- [x] **Step 2: Verify downstream RED before adapting manifests/API**

Run the focused core test. Expected: current wrapped error exposes the sentinel
or compilation later fails once the new builder signature is active.

- [x] **Step 3: Update package and downstream integration**

Set `qubit-json` to `0.4.0`; change core's exact path dependency to `=0.4.0`;
use `with_max_input_bytes(Some(Self::DEFAULT_MAX_JSON_BYTES))`. Let Cargo update
only the two in-scope lock files.

- [x] **Step 4: Synchronize public documentation**

Document private Options fields/getters/builders, default redaction, explicit
Detailed risk, three bug semantics, and version `0.4.0` in both READMEs, PRD,
and design document. Add a runnable crate-level rustdoc example for default
redaction and configured decoding.

- [x] **Step 5: Verify downstream GREEN**

Run:

```bash
cargo test --test mod entity_decoder
```

from `rs-llmsdk-core`. Expected: existing and new entity decoder tests pass.

---

### Task 8: Full verification and review

**Files:**
- Review all changed files in both in-scope repositories.

- [x] **Step 1: Run rs-json verification**

```bash
cargo test --all-targets --all-features
cargo test --doc
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features
./style-check.sh
cargo llvm-cov --all-features --all-targets --summary-only
cargo package --allow-dirty
```

Expected: every command exits zero, doctests are non-zero, and line coverage is
at least 98.46%.

- [x] **Step 2: Run direct downstream verification**

```bash
cargo test --test mod entity_decoder
cargo check --locked
```

Expected: both commands exit zero in `rs-llmsdk-core`.

- [x] **Step 3: Review changes and scope**

Run `git --no-pager diff --check`, inspect both repository diffs, confirm no
OpenAI provider files changed, and confirm no unrelated refactor or performance
work entered the patch.

- [x] **Step 4: Report evidence and remaining exclusions**

Summarize privacy semantics, root causes, changed public API, exact test counts,
coverage, package result, downstream verification, and the deliberately
deferred performance/provider issues. Do not commit without a separate explicit
request.
