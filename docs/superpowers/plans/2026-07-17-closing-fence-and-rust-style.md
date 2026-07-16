# Closing Fence and Rust Style Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reject invalidly indented Markdown closing fences and align all Rust file headers and parameter documentation with the approved repository standard.

**Architecture:** Keep the existing lightweight normalizer and tighten only its final closing-line recognition. Protect both optional and required closing policies with external regression tests, then apply mechanical style changes without altering public APIs or dependencies.

**Tech Stack:** Rust 2024, serde_json, the existing consolidated external test target, and the repository validation scripts.

## Global Constraints

- Preserve every public API, module path, dependency, and normalization stage.
- Recognize a closing fence only with zero to three leading ASCII spaces.
- Preserve the existing outer Unicode-whitespace trim before fence recognition.
- Use the exact approved seven-line copyright header at byte zero in all 29 Rust files.
- Rename all 49 `# Arguments` headings under `src` to `# Parameters` without changing their prose.
- Follow test-driven development: verify the regression tests fail before changing production code.
- Use English grouped commit messages as previously requested; do not push.

---

### Task 1: Closing-fence regression and parser correction

**Files:**
- Modify: `tests/internal/lenient_json_normalizer_tests.rs:201`
- Modify: `src/internal/lenient_json_normalizer.rs:385-403`

**Interfaces:**
- Consumes: `LenientJsonDecoder::decode_value(&self, input: &str) -> Result<serde_json::Value, JsonDecodeError>` and `MarkdownFenceClosing::{Optional, Required}`.
- Produces: unchanged public interfaces; `strip_markdown_closing_fence(content: &str, opening_fence: MarkdownFence) -> Option<&str>` rejects invalid closing-line whitespace.

- [ ] **Step 1: Add failing optional-policy regression coverage**

Add this external test after `test_decode_value_strips_code_fence_with_indented_closing_fence`:

```rust
#[test]
fn test_decode_value_rejects_invalid_closing_fence_indentation_with_optional_policy()
{
    let decoder = LenientJsonDecoder::default();
    for closing_line in [
        "    ```",
        "\t```",
        "\u{00a0}```",
    ] {
        let input = format!("```json\n{{\"a\":1}}\n{closing_line}");
        let error = decoder.decode_value(&input).expect_err(
            "invalid closing-fence whitespace must remain in the JSON body",
        );
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    }
}
```

- [ ] **Step 2: Add failing required-policy regression coverage**

Add this adjacent external test:

```rust
#[test]
fn test_decode_value_rejects_invalid_closing_fence_indentation_with_required_policy()
{
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_markdown_fence_policy(
            MarkdownFencePolicy::Any {
                closing: MarkdownFenceClosing::Required,
            },
        ),
    );
    for closing_line in [
        "    ```",
        "\t```",
        "\u{00a0}```",
    ] {
        let input = format!("```json\n{{\"a\":1}}\n{closing_line}");
        let error = decoder.decode_value(&input).expect_err(
            "required mode must reject invalid closing-fence whitespace",
        );
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    }
}
```

- [ ] **Step 3: Run the focused tests and verify RED**

Run:

```bash
cargo test --test tests invalid_closing_fence_indentation -- --nocapture
```

Expected: both new tests fail because the current `.trim()` accepts the first four-space closing line and `expect_err` receives `Ok(Object {"a": Number(1)})`.

- [ ] **Step 4: Implement the minimal closing-line grammar**

Replace the body of `strip_markdown_closing_fence` with:

```rust
fn strip_markdown_closing_fence(
    content: &str,
    opening_fence: MarkdownFence,
) -> Option<&str> {
    let trimmed_end = content.trim_end_matches(|ch| {
        matches!(ch, ' ' | '\t' | '\n' | '\r')
    });
    let closing_line_start = trimmed_end
        .rfind('\n')
        .max(trimmed_end.rfind('\r'))
        .map_or(0, |index| index + 1);
    let closing_line = &trimmed_end[closing_line_start..];
    let indent_len = closing_line
        .bytes()
        .take_while(|byte| *byte == b' ')
        .count();
    if indent_len > 3 {
        return None;
    }
    let marker_line = &closing_line[indent_len..];
    let closing_len =
        Self::same_marker_fence_len(marker_line, opening_fence.marker)?;
    if closing_len == marker_line.len()
        && closing_len >= opening_fence.marker_len
    {
        Some(&content[..closing_line_start])
    } else {
        None
    }
}
```

- [ ] **Step 5: Run the focused tests and verify GREEN**

Run:

```bash
cargo test --test tests invalid_closing_fence_indentation -- --nocapture
```

Expected: 2 passed, 0 failed.

- [ ] **Step 6: Run the full consolidated test target**

Run:

```bash
cargo test --test tests
```

Expected: all tests pass with no warnings.

- [ ] **Step 7: Commit the behavioral fix**

```bash
git add src/internal/lenient_json_normalizer.rs tests/internal/lenient_json_normalizer_tests.rs
git commit -m "fix(json): enforce closing fence indentation"
```

---

### Task 2: Repository-wide Rust header and Rustdoc alignment

**Files:**
- Modify: every `*.rs` file under `src`, `tests`, `benches`, and `fuzz/fuzz_targets`.
- Modify Rustdoc headings in: `src/internal/control_character_escaper.rs`, `src/internal/lenient_json_normalizer.rs`, `src/json_decode_error.rs`, `src/json_decode_error_kind.rs`, `src/json_decode_options.rs`, `src/json_decode_stage.rs`, `src/json_top_level_kind.rs`, and `src/lenient_json_decoder.rs`.

**Interfaces:**
- Consumes: the existing source and external-test layout.
- Produces: byte-zero standard headers in 29 Rust files and `# Parameters` headings for all 49 documented parameter lists; compiled behavior remains unchanged.

- [ ] **Step 1: Replace every Rust file header with the exact standard**

Apply this exact header, with no blank line before it, to every file returned by `rg --files -g '*.rs'`:

```rust
// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
```

The complete file set is:

```text
benches/decoder_bench.rs
fuzz/fuzz_targets/decoder.rs
src/error_privacy_policy.rs
src/internal/control_character_escaper.rs
src/internal/lenient_json_normalizer.rs
src/internal/markdown_fence.rs
src/internal/mod.rs
src/json_decode_error.rs
src/json_decode_error_kind.rs
src/json_decode_options.rs
src/json_decode_stage.rs
src/json_top_level_kind.rs
src/lenient_json_decoder.rs
src/lib.rs
src/markdown_fence_closing.rs
src/markdown_fence_policy.rs
tests/error_privacy_policy_tests.rs
tests/internal/lenient_json_normalizer_tests.rs
tests/internal/mod.rs
tests/json_decode_error_kind_tests.rs
tests/json_decode_error_tests.rs
tests/json_decode_options_tests.rs
tests/json_decode_stage_tests.rs
tests/json_top_level_kind_tests.rs
tests/lenient_json_decoder_tests.rs
tests/lib_tests.rs
tests/markdown_fence_closing_tests.rs
tests/markdown_fence_policy_tests.rs
tests/mod.rs
```

- [ ] **Step 2: Rename parameter-section headings**

In the eight source files listed above, replace every exact `/// # Arguments` line with `/// # Parameters`. Do not modify parameter prose or any `# Returns` and `# Errors` sections.

- [ ] **Step 3: Verify the mechanical invariants**

Run:

```bash
test "$(rg -l '^//    Copyright \(c\) 2025 - 2026 Haixing Hu\.$' -g '*.rs' | wc -l | tr -d ' ')" = "29"
test "$(rg -n '/// # Parameters' src | wc -l | tr -d ' ')" = "49"
test -z "$(rg -n '/// # Arguments|Copyright \(c\) 2026 Haixing Hu\.' -g '*.rs' src tests benches fuzz/fuzz_targets || true)"
```

Expected: exit 0 with no output.

- [ ] **Step 4: Inspect the style-only diff**

Run:

```bash
git --no-pager diff --check
git --no-pager diff -- src tests benches fuzz/fuzz_targets
```

Expected: no whitespace errors; changes outside the Task 1 parser/tests are limited to headers and `Arguments` → `Parameters` headings.

- [ ] **Step 5: Commit the style alignment**

```bash
git add src tests benches fuzz/fuzz_targets
git commit -m "style(json): standardize Rust headers and docs"
```

---

### Task 3: Repository validation

**Files:**
- Modify: `docs/superpowers/specs/2026-07-17-closing-fence-and-rust-style-design.md`
- Modify: `docs/superpowers/plans/2026-07-17-closing-fence-and-rust-style.md`
- Inspect: all changes made by `align-ci.sh` before proceeding.

**Interfaces:**
- Consumes: the completed behavioral and style changes from Tasks 1 and 2.
- Produces: a committed record of the user-confirmed outer-whitespace boundary
  and CI-equivalent verification evidence for the completed changes.

- [ ] **Step 1: Commit the confirmed specification correction**

Inspect the two documentation changes and commit only them:

```bash
git --no-pager diff -- docs/superpowers/specs/2026-07-17-closing-fence-and-rust-style-design.md docs/superpowers/plans/2026-07-17-closing-fence-and-rust-style.md
git add docs/superpowers/specs/2026-07-17-closing-fence-and-rust-style-design.md docs/superpowers/plans/2026-07-17-closing-fence-and-rust-style.md
git commit -m "docs(json): clarify outer whitespace handling"
```

Expected: the commit only records that terminal Unicode whitespace is removed
by the existing outer trim, while four spaces, a tab, or Unicode whitespace
before the closing marker remain invalid indentation.

- [ ] **Step 2: Run repository alignment first**

Run:

```bash
./align-ci.sh
```

Expected: exit 0. Immediately inspect `git status --short` and `git --no-pager diff`; retain only in-scope formatting/alignment changes.

- [ ] **Step 3: Run CI-equivalent validation**

Run:

```bash
./ci-check.sh
```

Expected: exit 0 with formatting, compilation, Clippy, tests, doctests, and the configured coverage threshold passing.

- [ ] **Step 4: Run coverage JSON only if CI reports coverage below threshold**

If and only if Step 3 reports coverage below its threshold, run:

```bash
./coverage.sh json
```

Expected: exit 0 and a per-file coverage report identifying any remaining uncovered branch. Do not run this command when Step 3 meets the configured threshold.

- [ ] **Step 5: Recheck exact scope and history**

Run:

```bash
git status --short
git --no-pager diff --check
git --no-pager log -4 --oneline
```

Expected: an empty status after the Task 1 and Task 2 commits; no whitespace errors are reported.

- [ ] **Step 6: Verify the final repository state**

Run:

```bash
git status --short
git --no-pager log -3 --oneline
```

Expected: an empty status and two English grouped implementation commits for behavior and style.
