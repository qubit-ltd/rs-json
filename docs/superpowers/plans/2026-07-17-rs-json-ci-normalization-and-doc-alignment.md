# RS-JSON CI, Normalization, and Documentation Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align local and hosted CI revisions, simplify the behavior-preserving normalization pipeline, close line-ending coverage gaps, split focused escaper tests, and synchronize Rustdoc and bilingual project documentation.

**Architecture:** Keep `LenientJsonNormalizer` as the single private normalization orchestrator and `ControlCharacterEscaper` as its focused state machine. Derive the hosted reusable-workflow pin from the `.rs-ci` submodule revision, protect refactoring through public decoder behavior, and keep production visibility unchanged.

**Tech Stack:** Rust 2024, serde/serde_json, Cargo integration tests, Git submodules and GitHub Actions, repository shell validation scripts, Criterion, and cargo-fuzz.

## Global Constraints

- Limit repository changes to `rs-json`; do not modify downstream `rs-*` repositories.
- Run `./update-submodule.sh` so `.rs-ci` advances to the latest commit on its configured `main` branch.
- Pin `.github/workflows/ci.yml` to the exact 40-character `.rs-ci` commit ID; never leave the reusable workflow on `@main`.
- Preserve public APIs, module paths, dependencies, and observable normalization behavior; breaking changes are permitted only if an implementation necessity is discovered and documented.
- Treat the CRLF and lone-CR tests as characterization tests that pass before and after refactoring; do not manufacture a failing behavior.
- Observe private behavior through public APIs; do not widen production visibility and do not add inline test modules.
- Every new Rust file starts at byte zero with the approved seven-line `Copyright (c) 2025 - 2026 Haixing Hu.` header.
- Use the exact bilingual badge blocks and final-section templates from `reviewing-rust-code-style/references/readme-structure.md`.
- Run `./align-ci.sh` before `./ci-check.sh`; run `./coverage.sh json` only if CI reports coverage below its threshold.
- Group commits by change intent, use English Angular-style commit messages, preserve unrelated user changes, and do not push.
- Work in `.worktrees/ci-normalization-doc-alignment`, fast-forward the completed branch into the original `dev-starfish` branch, then remove the worktree and merged feature branch.

## File Structure

- `.rs-ci`: exact shared CI implementation revision used locally.
- `.github/workflows/ci.yml`: hosted reusable workflow pinned to the same revision.
- `tests/internal/lenient_json_normalizer_tests.rs`: normalization-pipeline and fence behavior observed through `LenientJsonDecoder`.
- `tests/internal/control_character_escaper_tests.rs`: mirrored external test module for control-character state-machine behavior observed through public decoding APIs.
- `tests/internal/mod.rs`: registers both mirrored test modules.
- `src/internal/lenient_json_normalizer.rs`: removes the redundant final trim, performs one first-line-break scan, and documents the actual closing-fence grammar.
- `src/json_top_level_kind.rs`: documents pre-parse top-level classification as provisional.
- `doc/json_prd.zh_CN.md`: shortened pipeline and complete test inventory.
- `doc/json_design.zh_CN.md`: exact helper name, fence grammar, pipeline, directory tree, and split test responsibilities.
- `README.md`: six badges, eight-step pipeline, and exact required English ending.
- `README.zh_CN.md`: six badges, dedicated untrusted-input limit guidance, eight-step pipeline, and exact required Chinese ending.

---

### Task 1: Update and Pin the Shared CI Revision

**Files:**
- Modify: `.rs-ci`
- Modify: `.github/workflows/ci.yml:18`

**Interfaces:**
- Consumes: `.gitmodules` tracking configuration and `update-submodule.sh` default remote-update behavior.
- Produces: one full Git commit ID used both as the `.rs-ci` gitlink and the suffix of `qubit-ltd/rs-ci/.github/workflows/rust-ci.yml@...`.

- [ ] **Step 1: Create the isolated implementation worktree**

Invoke `superpowers:using-git-worktrees`, verify that `.worktrees` is ignored, and create the branch and worktree from the confirmed-plan commit:

```bash
git check-ignore -q .worktrees
git worktree add .worktrees/ci-normalization-doc-alignment \
  -b codex/rs-json-ci-normalization-doc-alignment
```

Expected: the new worktree is attached to `codex/rs-json-ci-normalization-doc-alignment`, while the original checkout remains on `dev-starfish`.

- [ ] **Step 2: Establish a clean baseline in the worktree**

Run from `.worktrees/ci-normalization-doc-alignment`:

```bash
git status --short --branch
cargo test --test tests
```

Expected: the feature branch is clean and the integration test binary exits 0.

- [ ] **Step 3: Update `.rs-ci` through the authorized script**

```bash
./update-submodule.sh
git status --short
git --no-pager diff --submodule=log -- .rs-ci
git -C .rs-ci rev-parse HEAD
```

Expected: the script exits 0, `.rs-ci` points at the latest configured remote `main` commit, and `rev-parse` prints its 40-character commit ID.

- [ ] **Step 4: Pin the hosted workflow to the derived revision**

Store the exact runtime value:

```bash
RS_CI_SHA=$(git -C .rs-ci rev-parse HEAD)
printf '%s\n' "$RS_CI_SHA"
```

Use `apply_patch` to replace only `@main`. The resulting YAML line has this exact shape, with `RS_CI_SHA` replaced by the printed value rather than retained as text:

```yaml
    uses: qubit-ltd/rs-ci/.github/workflows/rust-ci.yml@RS_CI_SHA
```

- [ ] **Step 5: Verify that the two CI references are identical**

```bash
submodule_sha=$(git -C .rs-ci rev-parse HEAD)
workflow_sha=$(sed -n \
  's|.*rust-ci.yml@\([0-9a-f]\{40\}\).*|\1|p' \
  .github/workflows/ci.yml)
test -n "$workflow_sha"
test "$submodule_sha" = "$workflow_sha"
git diff --check
```

Expected: every command exits 0 and both variables contain the same full commit ID.

- [ ] **Step 6: Commit the CI revision as one change group**

```bash
git add .rs-ci .github/workflows/ci.yml
git --no-pager diff --cached --submodule=log
git commit -m "chore(ci): update and pin rs-ci"
```

Expected: one English commit containing only the gitlink update and workflow pin.

---

### Task 2: Add Complete First-Line-Ending Characterization

**Files:**
- Modify: `tests/internal/lenient_json_normalizer_tests.rs:123-150`

**Interfaces:**
- Consumes: `LenientJsonDecoder::default()` and its existing Markdown-fence policy.
- Produces: regressions named `test_decode_value_strips_code_fence_with_crlf_line_endings` and `test_decode_value_strips_code_fence_with_cr_only_line_endings`.

- [ ] **Step 1: Add both characterization tests before refactoring**

Insert after `test_decode_value_strips_code_fence_with_closing_fence`:

```rust
#[test]
fn test_decode_value_strips_code_fence_with_crlf_line_endings() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\r\n{\"a\":1}\r\n```")
        .expect("default decoder should accept CRLF fenced JSON");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_strips_code_fence_with_cr_only_line_endings() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\r{\"a\":1}\r```")
        .expect("default decoder should accept CR-only fenced JSON");
    assert_eq!(value, json!({"a": 1}));
}
```

- [ ] **Step 2: Verify that existing behavior supports both cases**

```bash
cargo test --test tests \
  internal::lenient_json_normalizer_tests::test_decode_value_strips_code_fence_with_cr
```

Expected: both characterization tests pass. This is a behavior-preserving coverage step, not an artificial RED phase.

- [ ] **Step 3: Commit the line-ending coverage**

```bash
git add tests/internal/lenient_json_normalizer_tests.rs
git diff --cached --check
git commit -m "test(json): cover CRLF and CR fences"
```

Expected: the commit contains only the two new regressions.

---

### Task 3: Split Control-Character Escaper Coverage

**Files:**
- Create: `tests/internal/control_character_escaper_tests.rs`
- Modify: `tests/internal/mod.rs:8-10`
- Modify: `tests/internal/lenient_json_normalizer_tests.rs:496-605`

**Interfaces:**
- Consumes: public `LenientJsonDecoder`, `JsonDecodeOptions`, and `JsonDecodeErrorKind`; production visibility remains unchanged.
- Produces: a mirrored test module for `src/internal/control_character_escaper.rs`; normalizer tests retain whitespace/repair ordering and fence-pipeline interactions.

- [ ] **Step 1: Create the mirrored escaper test module**

Create `tests/internal/control_character_escaper_tests.rs` using `apply_patch`. Start with the exact header and imports:

```rust
// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests control-character escaping through public decoder behavior.

use serde_json::json;

use qubit_json::{
    JsonDecodeErrorKind,
    JsonDecodeOptions,
    LenientJsonDecoder,
};
```

Move the seven complete tests below from `lenient_json_normalizer_tests.rs`.
The new file therefore contains the imports above followed by this exact code:

```rust
#[test]
fn test_decode_value_preserves_existing_escapes() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("{\"text\":\"a\\nb\"}")
        .expect("existing JSON escapes should remain valid");
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_escapes_control_chars_in_strings() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("{\"text\":\"a\nb\"}").expect(
        "default decoder should escape control characters inside strings",
    );
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_can_disable_control_char_escaping() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_escape_control_chars_in_strings(false),
    );
    let error = decoder
        .decode_value("{\"text\":\"a\nb\"}")
        .expect_err("control characters should remain invalid JSON when escaping is disabled");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_covers_all_supported_control_char_escapes() {
    let control_chars = [
        '\u{0000}', '\u{0001}', '\u{0002}', '\u{0003}', '\u{0004}', '\u{0005}',
        '\u{0006}', '\u{0007}', '\u{0008}', '\u{0009}', '\u{000a}', '\u{000b}',
        '\u{000c}', '\u{000d}', '\u{000e}', '\u{000f}', '\u{0010}', '\u{0011}',
        '\u{0012}', '\u{0013}', '\u{0014}', '\u{0015}', '\u{0016}', '\u{0017}',
        '\u{0018}', '\u{0019}', '\u{001a}', '\u{001b}', '\u{001c}', '\u{001d}',
        '\u{001e}', '\u{001f}',
    ];
    let control_text: String = control_chars.into_iter().collect();
    let json_input = format!("{{\"text\":\"{control_text}\"}}");

    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value(&json_input).expect(
        "all supported ASCII control characters should be escaped successfully",
    );
    assert_eq!(value, json!({"text": control_text}));
}

#[test]
fn test_decode_value_escapes_control_char_after_unmatched_backslash() {
    let decoder = LenientJsonDecoder::default();

    for code_point in 0_u32..=0x1f {
        let control = char::from_u32(code_point)
            .expect("ASCII control code points should be valid chars");
        let mut json_input = String::from("{\"text\":\"");
        json_input.push('\\');
        json_input.push(control);
        json_input.push_str("\"}");

        let value = decoder.decode_value(&json_input).unwrap_or_else(|error| {
            panic!(
                "control U+{code_point:04X} after an unmatched backslash should be repaired: {error}"
            )
        });
        assert_eq!(
            value,
            json!({"text": control.to_string()}),
            "unexpected decoded value for U+{code_point:04X}",
        );
    }
}

#[test]
fn test_decode_value_escapes_control_chars_after_odd_and_even_backslashes() {
    let decoder = LenientJsonDecoder::default();

    for control in ['\n', '\u{0000}'] {
        for backslash_count in 1..=4 {
            let mut json_input = String::from("{\"text\":\"");
            json_input.extend(std::iter::repeat_n('\\', backslash_count));
            json_input.push(control);
            json_input.push_str("\"}");

            let value = decoder.decode_value(&json_input).unwrap_or_else(|error| {
                panic!(
                    "{backslash_count} backslashes before {control:?} should be repaired: {error}"
                )
            });
            let mut expected = "\\".repeat(backslash_count / 2);
            expected.push(control);
            assert_eq!(
                value,
                json!({"text": expected}),
                "unexpected decoded value for {backslash_count} backslashes before {control:?}",
            );
        }
    }
}

#[test]
fn test_decode_value_leaves_non_whitespace_controls_outside_strings_invalid() {
    let error = LenientJsonDecoder::default()
        .decode_value("\u{0001}{\"text\":\"value\"}")
        .expect_err("a raw control character outside a JSON string must not be repaired");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}
```

Remove these exact functions from `lenient_json_normalizer_tests.rs`; do not change their bodies while moving them.

- [ ] **Step 2: Register both mirrored modules**

Update the body of `tests/internal/mod.rs` to:

```rust
//! Tests for private normalization behavior observed through public APIs.

mod control_character_escaper_tests;
mod lenient_json_normalizer_tests;
```

- [ ] **Step 3: Keep pipeline interaction tests in the normalizer module**

After removing the seven moved functions, retain these functions in `lenient_json_normalizer_tests.rs`:

```rust
fn test_decode_value_trims_surrounding_whitespace_by_default()
fn test_decode_value_with_trim_disabled_and_escape_enabled_still_decodes_owned_output()
fn test_decode_value_trims_owned_output_after_repair()
```

- [ ] **Step 4: Verify both responsibilities independently**

```bash
cargo test --test tests internal::control_character_escaper_tests
cargo test --test tests internal::lenient_json_normalizer_tests
```

Expected: both filtered runs exit 0; no existing test is lost or duplicated.

- [ ] **Step 5: Commit the test-organization change**

```bash
git add tests/internal/control_character_escaper_tests.rs \
  tests/internal/lenient_json_normalizer_tests.rs \
  tests/internal/mod.rs
git diff --cached --check
git commit -m "test(json): split control character escaper coverage"
```

Expected: the commit reorganizes tests only and does not touch production visibility.

---

### Task 4: Simplify the Normalization Pipeline

**Files:**
- Modify: `src/internal/lenient_json_normalizer.rs:85-112,183-229,322-412`
- Modify: `tests/internal/lenient_json_normalizer_tests.rs`

**Interfaces:**
- Consumes: the CRLF/lone-CR characterization tests and existing public decoder regressions.
- Produces: unchanged `fn normalize<'a>(&self, input: &'a str) -> Result<Cow<'a, str>, JsonDecodeError>` behavior without `trim_cow_if_enabled`, plus a one-scan `fn first_line_break(input: &str) -> Option<(usize, usize)>`.

- [ ] **Step 1: Remove the redundant post-escape trim**

Change the end of `normalize` to:

```rust
        let input = ControlCharacterEscaper::escape(
            input,
            self.options.escape_control_chars_in_strings(),
        );

        if input.is_empty() {
            Err(JsonDecodeError::empty_input(
                raw_input_bytes,
                Some(input.len()),
                self.options.error_privacy_policy(),
            ))
        } else {
            Ok(input)
        }
```

Delete the complete `trim_cow_if_enabled` method and its Rustdoc. Keep `trim_if_enabled` unchanged.

- [ ] **Step 2: Replace two delimiter searches with one byte scan**

Replace `first_line_break` with:

```rust
    fn first_line_break(input: &str) -> Option<(usize, usize)> {
        let bytes = input.as_bytes();
        let line_end = bytes
            .iter()
            .position(|byte| matches!(*byte, b'\n' | b'\r'))?;
        let next_line_start = if bytes[line_end] == b'\r'
            && bytes.get(line_end + 1) == Some(&b'\n')
        {
            line_end + 2
        } else {
            line_end + 1
        };
        Some((line_end, next_line_start))
    }
```

- [ ] **Step 3: Correct the closing-fence helper Rustdoc**

Replace its `# Returns` prose with:

```rust
    /// `Some(body)` when, after ignoring trailing ASCII spaces, tabs, CRs, and
    /// LFs, the final line begins with zero to three ASCII spaces and contains
    /// a compatible closing marker run; otherwise, `None`.
```

- [ ] **Step 4: Rename the misleading pipeline regression**

Replace the retained test with:

```rust
#[test]
fn test_decode_value_trims_before_control_character_repair() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n  {\"text\":\"a\nb\"}  \n```")
        .expect(
            "outer whitespace should be removed before repair allocates an owned string",
        );
    assert_eq!(value, json!({"text": "a\nb"}));
}
```

- [ ] **Step 5: Verify the behavior-preserving refactor**

```bash
cargo test --test tests internal::lenient_json_normalizer_tests
cargo test --test tests internal::control_character_escaper_tests
cargo test --test tests
```

Expected: every command exits 0, including both line-ending regressions and all control-character cases.

- [ ] **Step 6: Confirm obsolete code is gone and commit**

```bash
test -z "$(rg -n 'trim_cow_if_enabled' src tests || true)"
git add src/internal/lenient_json_normalizer.rs \
  tests/internal/lenient_json_normalizer_tests.rs
git diff --cached --check
git commit -m "refactor(json): simplify normalization pipeline"
```

Expected: the commit contains the normalizer simplification, accurate helper Rustdoc, and test rename.

---

### Task 5: Align Rustdoc and Canonical Documents

**Files:**
- Modify: `src/json_top_level_kind.rs:57-66`
- Modify: `doc/json_prd.zh_CN.md:102,171-186`
- Modify: `doc/json_design.zh_CN.md:224-263,280-343`

**Interfaces:**
- Consumes: the final eight-stage pipeline and split test layout from Tasks 3 and 4.
- Produces: Rustdoc and canonical documents that describe current behavior without changing compiled behavior.

- [ ] **Step 1: Mark top-level text classification as provisional**

Replace the summary and add the non-validation statement:

```rust
    /// Provisionally classifies normalized JSON text by its first JSON token.
    ///
    /// This helper does not validate JSON syntax. The decoder uses the result
    /// only as a pre-parse top-level-kind hint.
```

Keep the existing parameters, returns, signature, and implementation.

- [ ] **Step 2: Update the PRD pipeline and test inventory**

Use this fixed order in `doc/json_prd.zh_CN.md`:

```text
require_within_size_limit -> require_non_empty -> trim -> strip_bom -> trim -> strip_fence -> trim -> escape_control_chars
```

Add this path beside the existing normalizer test path:

```markdown
  - `tests/internal/control_character_escaper_tests.rs`
```

- [ ] **Step 3: Update the design document's exact implementation model**

Replace pipeline steps 8 through 10 with:

```markdown
8. `ControlCharacterEscaper::escape(input, enabled)`：可配置转义字符串内控制字符。
9. 最终空值检查并返回 `Cow<'_, str>`。
```

Add the exact closing grammar:

```markdown
  - closing fence 前最多允许 3 个 ASCII 空格缩进；tab、非 ASCII 空白或 4 个及以上空格不构成 closing fence。
  - closing marker 后仅允许 ASCII 空格或 tab；marker 类型必须相同，且长度不得短于 opening fence。
```

Change the escaper subsection name to `ControlCharacterEscaper::escape`, add `control_character_escaper_tests.rs` to the directory tree, and replace the normalization-test inventory with:

```markdown
- `tests/internal/control_character_escaper_tests.rs`：通过公开 decoder 行为覆盖字符串状态、已有转义、全部 C0 控制字符、反斜杠奇偶语义和字符串外控制字符。
- `tests/internal/lenient_json_normalizer_tests.rs`：通过公开 decoder 行为覆盖 BOM、围栏换行、空输入诊断、trim 与控制字符修复的管线交互，不扩大内部实现可见性。
```

- [ ] **Step 4: Audit and commit canonical documentation**

```bash
test -z "$(rg -n 'trim_cow_if_enabled|escape_control_chars_in_json_strings' \
  src doc || true)"
git diff --check
git add src/json_top_level_kind.rs doc/json_prd.zh_CN.md \
  doc/json_design.zh_CN.md
git commit -m "docs(json): align normalization documentation"
```

Expected: one English documentation commit with no compiled behavior change.

---

### Task 6: Align Both README Files

**Files:**
- Modify: `README.md:1-9,202-266`
- Modify: `README.zh_CN.md:1-9,150-245`

**Interfaces:**
- Consumes: repository metadata `qubit-json`, Rust 1.94, and `https://github.com/qubit-ltd/rs-json`.
- Produces: exact six-badge blocks, eight-step normalization pipelines, dedicated Chinese untrusted-input guidance, and exact required final four sections.

- [ ] **Step 1: Apply the exact six-badge blocks**

Remove the docs.rs badge. The English block is:

```markdown
[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)
```

The Chinese block is:

```markdown
[![Rust CI](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-json/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-json/coverage-badge.json)](https://qubit-ltd.github.io/rs-json/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-json.svg?color=blue)](https://crates.io/crates/qubit-json)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)
```

- [ ] **Step 2: Add dedicated Chinese untrusted-input guidance**

Insert after the Chinese custom-options example:

````markdown
### 为不可信来源设置输入上限

`JsonDecodeOptions::default()` 有意不设置 `max_input_bytes`，避免库强加与
应用场景无关的限制。当输入跨越信任边界时，应根据调用方的内存和延迟预算配置上限。

```rust
use qubit_json::{JsonDecodeOptions, LenientJsonDecoder};

let decoder = LenientJsonDecoder::new(
    JsonDecodeOptions::default().with_max_input_bytes(Some(1_048_576)),
);
let value = decoder.decode_value("{\"ok\":true}")?;

assert_eq!(value["ok"], true);
# Ok::<(), qubit_json::JsonDecodeError>(())
```
````

- [ ] **Step 3: End both normalization pipelines at step 8**

Delete English step 9 `trim surrounding whitespace again` and Chinese step 9 `再次裁剪首尾空白`. Do not change steps 1 through 8.

- [ ] **Step 4: Apply the exact English final four sections**

Move Alignment Notes and Development Validation before this block, remove the earlier short License section, and end `README.md` exactly with:

````markdown
## Testing

```bash
# Core API with the default empty feature set
cargo test --no-default-features

# Core API plus regex validation
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./align-ci.sh` to format code and
`./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-json](https://github.com/qubit-ltd/rs-json)
````

- [ ] **Step 5: Apply the exact Chinese final four sections**

Keep 对齐说明 and 开发验证 before this block, replace the current short 许可证 ending, and end `README.zh_CN.md` exactly with:

````markdown
## 测试

```bash
# 使用默认的空 feature 集测试核心 API
cargo test --no-default-features

# 测试核心 API 和正则校验
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-json](https://github.com/qubit-ltd/rs-json)
````

- [ ] **Step 6: Audit README structure and commit**

```bash
test "$(rg -c '^\[!\[' README.md)" -eq 6
test "$(rg -c '^\[!\[' README.zh_CN.md)" -eq 6
test -z "$(rg -n 'docs\.rs' README.md README.zh_CN.md || true)"
git diff --check
git add README.md README.zh_CN.md
git commit -m "docs(readme): standardize bilingual metadata"
```

Expected: exactly six badges in each language and the final four H2 headings match the authoritative templates with no later H2 content.

---

### Task 7: Validate, Integrate, and Remove the Worktree

**Files:**
- Verify: all files changed in Tasks 1 through 6
- Possibly modify: only files rewritten by `./align-ci.sh`, after explicit diff inspection

**Interfaces:**
- Consumes: the complete feature branch and repository-prescribed validation scripts.
- Produces: a clean, verified `dev-starfish` branch containing grouped English commits, with no remaining feature worktree or branch.

- [ ] **Step 1: Run repository alignment first and inspect its output**

```bash
./align-ci.sh
git status --short
git --no-pager diff --check
git --no-pager diff
```

Expected: alignment exits 0. Every generated change is either already in scope or identified as out of scope before proceeding.

- [ ] **Step 2: Commit any alignment-only rewrite**

If Step 1 changed tracked files, stage only the inspected formatting changes:

```bash
git add src tests README.md README.zh_CN.md doc .github
git --no-pager diff --cached --check
git commit -m "style(json): apply repository alignment"
```

Expected: either no commit is needed, or the commit contains formatting/alignment output only.

- [ ] **Step 3: Run CI-equivalent validation**

```bash
./ci-check.sh
```

Expected: exit 0 across formatting, linting, tests, documentation, coverage, and repository checks. Only if CI reports coverage below threshold, run:

```bash
./coverage.sh json
```

Expected when conditionally required: exit 0 and JSON coverage output identifying the remaining uncovered branch before any additional test change.

- [ ] **Step 4: Compile the Criterion benchmark**

```bash
cargo bench --bench decoder_bench --no-run
```

Expected: exit 0 and a compiled `decoder_bench` executable. No measurement is required because benchmarked behavior is unchanged.

- [ ] **Step 5: Build the decoder fuzz target with the workflow toolchain**

```bash
cargo fuzz --version || cargo install cargo-fuzz --version 0.13.2 --locked
rustup toolchain install nightly-2026-06-05 --profile minimal
cd fuzz
cargo +nightly-2026-06-05 fuzz build decoder
cd ..
```

Expected: cargo-fuzz is available and the decoder fuzz target builds with nightly-2026-06-05.

- [ ] **Step 6: Perform final branch and CI-pin audits**

```bash
git status --short --branch
git --no-pager diff HEAD
git --no-pager log --oneline --decorate -10
submodule_sha=$(git -C .rs-ci rev-parse HEAD)
workflow_sha=$(sed -n \
  's|.*rust-ci.yml@\([0-9a-f]\{40\}\).*|\1|p' \
  .github/workflows/ci.yml)
test "$submodule_sha" = "$workflow_sha"
```

Expected: the feature worktree is clean, there is no uncommitted diff, commit groups are English and content-focused, and the CI revisions match.

- [ ] **Step 7: Fast-forward the original branch**

Run from `/home/starfish/working/qubit/rust-common/rs-json`:

```bash
git status --short --branch
git merge --ff-only codex/rs-json-ci-normalization-doc-alignment
```

Expected: `dev-starfish` fast-forwards without a merge commit or conflict. If Git reports a conflict or non-fast-forward state, stop and ask the user.

- [ ] **Step 8: Verify the merged result**

```bash
./update-submodule.sh --no-remote
./ci-check.sh
git status --short --branch
git --no-pager log --oneline --decorate -10
```

Expected: the original checkout's `.rs-ci` worktree matches the newly recorded
gitlink, CI exits 0, and the checkout is clean on `dev-starfish`.

- [ ] **Step 9: Remove the merged worktree and feature branch**

First confirm the worktree is clean:

```bash
git -C .worktrees/ci-normalization-doc-alignment status --short
git worktree remove .worktrees/ci-normalization-doc-alignment
```

If Git refuses only because the clean linked worktree contains the initialized `.rs-ci` submodule, remove that already-merged worktree with:

```bash
git worktree remove --force .worktrees/ci-normalization-doc-alignment
```

Then finish cleanup:

```bash
git worktree prune
git branch -d codex/rs-json-ci-normalization-doc-alignment
git worktree list
git status --short --branch
```

Expected: only the original checkout remains, the merged feature branch is deleted, and `dev-starfish` is clean. Do not push.
