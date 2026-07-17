# CI, Normalization, and Documentation Alignment Design

## Scope

This change addresses the remaining maintainability issues found in the latest
`rs-json` review. It updates the shared CI submodule, keeps local and hosted CI
on the same revision, simplifies behavior-preserving normalization code, fills
line-ending coverage gaps, separates state-machine tests from pipeline tests,
and corrects project documentation.

The work is limited to `rs-json`. No downstream `rs-*` repository is changed.
No public API or normalization behavior is intentionally changed. The user has
allowed breaking changes when necessary, but none is required by this design.

## CI Revision Alignment

Run the repository's `update-submodule.sh` script in the isolated implementation
worktree so `.rs-ci` advances to the latest commit on its configured `main`
branch. After the update, read the full commit ID from the `.rs-ci` checkout and
replace the floating `@main` reference in `.github/workflows/ci.yml` with that
exact ID.

This makes the reusable GitHub Actions workflow and the local CI scripts come
from one auditable revision. Future updates remain deliberate: rerun the same
script and update the workflow pin in the same change.

## Normalization Simplification

`LenientJsonNormalizer::normalize` already applies optional surrounding-
whitespace trimming before control-character escaping. Escaping can replace
characters inside JSON strings but cannot introduce new whitespace at either
edge. Therefore, the final `trim_cow_if_enabled` call and its helper are
redundant. Remove both while retaining the existing empty-input check after the
escape phase.

Opening-fence recognition currently finds `\n` and `\r` independently and then
selects the earlier position. Replace this with one byte scan for the first CR
or LF. The recognized inputs and the treatment of `\n`, `\r\n`, and lone `\r`
remain unchanged.

The existing test whose name claims that owned output is trimmed after repair
will be renamed to describe the actual ordering: outer whitespace is removed
before an owned control-character repair. Its assertions remain behaviorally
equivalent.

## Test Organization and Coverage

Add external regression tests for fenced JSON using:

- CRLF line endings throughout; and
- lone CR line endings throughout.

These are characterization tests for already-supported behavior, so they are
expected to pass before the one-scan refactor. Their purpose is to execute and
lock down the currently uncovered line-ending branches; no artificial failing
behavior will be introduced merely to create a RED result. The refactor is
accepted only if the focused tests continue to pass unchanged.

Create `tests/internal/control_character_escaper_tests.rs` and register it from
`tests/internal/mod.rs`. Move only tests that directly exercise the pure
`ControlCharacterEscaper` state machine into that file. Tests that exercise the
complete decoder or normalizer pipeline remain in
`lenient_json_normalizer_tests.rs`. This preserves external-test organization
while reducing the size and mixed responsibilities of the normalizer test
module.

## Rustdoc and Design Documentation

Correct the following descriptions without changing public behavior:

- Document `strip_markdown_closing_fence` using its actual closing-line grammar:
  zero to three leading ASCII spaces, the matching marker run, and only ASCII
  spaces or tabs after the marker.
- Describe `JsonTopLevelKind::of_normalized_json` as provisional
  classification of normalized text, not validation of JSON syntax.
- Update `doc/json_prd.zh_CN.md` and `doc/json_design.zh_CN.md` to remove the
  redundant final trim stage, name `ControlCharacterEscaper::escape`
  accurately, document the closing-fence indentation rule, and reflect the new
  test-module layout.

## README Alignment

Bring `README.md` and `README.zh_CN.md` into the repository's required
bilingual structure:

- keep exactly the six standard badges and remove the extra docs.rs badge;
- update the documented normalization pipeline so it matches the simplified
  implementation;
- add the dedicated Chinese explanation of limits for untrusted input so it
  matches the English guidance; and
- make the final four level-two sections exactly `Testing`, `License`,
  `Contributing`, and `Author` in English, and `测试`, `许可证`, `贡献`, and
  `作者` in Chinese, using the repository-standard content.

Project-specific development and alignment material remains before those final
sections.

## Implementation Isolation and Commit Structure

Perform implementation in a repository-local ignored worktree. Preserve any
unrelated user changes. Group commits by content and write all commit messages
in English, with separate commits for the CI revision, tests, normalization
refactoring, and documentation where the resulting diff supports that split.

After verification, fast-forward the feature branch into the original
repository's current branch, verify the merged result, remove the worktree, and
delete the merged feature branch. Do not push.

## Verification

Use focused tests while moving and changing the normalizer tests. Before
integration, run the repository-authorized sequence:

1. `./align-ci.sh`, then inspect any generated changes;
2. `./ci-check.sh`;
3. `./coverage.sh json` only if the CI coverage stage reports a threshold
   problem;
4. `cargo bench --bench decoder_bench --no-run`; and
5. build the decoder fuzz target with the toolchain and command used by the
   pinned CI revision.

After merging, rerun `./ci-check.sh` from the original checkout. Final status,
diff, submodule revision, workflow pin, and commit history must show that the
tree is clean, the two CI references match, and every requested change is
committed.

## Risks and Mitigations

The main CI risk is updating `.rs-ci` without updating the hosted workflow pin;
an explicit equality check between both full commit IDs prevents that drift.
The main code risk is changing fence recognition while simplifying the line-
break scan; CRLF, lone-CR, mixed-ending, and existing fence regressions protect
the behavior. Removing the final trim is low risk because the preceding escape
phase cannot alter either input edge, and the full decoder suite verifies this
invariant through public behavior.
