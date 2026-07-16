# Closing Fence and Rust Style Alignment Design

## Scope

This change corrects Markdown closing-fence recognition and aligns every
version-controlled Rust file in `rs-json` with the repository's approved
copyright header. It also renames the existing Rustdoc `# Arguments` headings
to `# Parameters`.

The public API, dependency set, normalization order, and all unrelated JSON
repair behavior remain unchanged.

## Closing-Fence Recognition

The existing lightweight fence parser remains in place. A candidate closing
line is recognized only when it has:

- zero to three leading ASCII spaces;
- a run of the same marker used by the opening fence;
- at least as many marker bytes as the opening fence; and
- only ASCII spaces or tabs after the marker run.

Four or more leading spaces, a leading tab, or other Unicode whitespace do not
form a closing fence. Under `MarkdownFenceClosing::Required`, the original
input is retained and parsing fails. Under `Optional`, only the opening fence
is stripped, leaving the invalid candidate in the JSON body so parsing also
fails.

No general-purpose Markdown parser or new dependency is introduced.

## Test Strategy

Implementation follows a red-green-refactor cycle:

1. Add external regression tests covering four-space indentation, leading tabs,
   and non-ASCII whitespace under optional and required closing policies.
2. Run the focused test target and confirm the new tests fail for the expected
   reason.
3. Apply the smallest parser change that enforces the closing-line grammar.
4. Re-run the focused tests, then the repository's authorized validation
   sequence.

Existing coverage for zero-to-three-space indentation, longer matching fences,
shorter closing fences, and mixed line endings remains unchanged.

## Style Alignment

All 29 Rust files under `src`, `tests`, `benches`, and `fuzz/fuzz_targets` use
the following seven lines starting at byte zero:

```text
// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
```

The 49 existing `# Arguments` headings under `src` become `# Parameters`.
Their prose, parameter lists, return documentation, and error documentation do
not otherwise change.

## Compatibility and Risks

The behavioral change only rejects closing lines that were previously accepted
because arbitrary leading whitespace was trimmed. Valid fences with up to three
leading spaces remain compatible. The style changes do not affect compiled
behavior or public paths.

The main implementation risk is accidentally trimming trailing blank lines or
changing optional-fence behavior. Focused regression tests protect both policy
branches, and the existing full test suite protects the wider normalization
pipeline.
