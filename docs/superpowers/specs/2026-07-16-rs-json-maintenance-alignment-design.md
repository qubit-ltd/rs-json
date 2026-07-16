# rs-json Maintenance Alignment Design

## Goal

Align `qubit-json` with the repository's Rust documentation, method-order,
inline, external-test-layout, design-document, and continuous-fuzzing rules
without changing its public API or decoding behavior.

## Scope and Compatibility

- Preserve every public type, method signature, option default, error kind,
  error stage, normalization rule, and privacy behavior in version `0.4.0`.
- Do not add prose extraction, byte-slice decoding, NDJSON, streaming JSON,
  public normalization helpers, or new repair policies.
- Keep all tests black-box through the public `qubit_json` API; do not widen
  production visibility for testing.
- Do not add runtime dependencies.

## Rust Source Alignment

### Rustdoc

Complete documentation for every production function and method, including
private helpers and trait implementations. Each item will describe its real
arguments, return semantics, and observable errors using `# Arguments`,
`# Returns`, and `# Errors` where applicable. `Option` accessors will define
both `Some` and `None`. No `# Panics` or `# Safety` section will be added unless
the implementation actually exposes such a condition.

### Method order

- Keep constructors first, ordered by visibility.
- In `JsonDecodeOptions`, keep each getter adjacent to its corresponding
  `with_*` builder, with the getter first.
- In `JsonDecodeError`, move restricted and private constructors before public
  accessors; keep non-constructor helpers after accessors.
- Preserve complete methods, including their Rustdoc and attributes, while
  moving them.

### Inline attributes

- Use `#[inline(always)]` for getters, value-style setters/builders, and pure
  forwarding methods.
- Use `#[inline]` for other short methods with limited control flow.
- Remove inline attributes from constructors that format strings or build
  larger diagnostics.
- Do not add inline attributes to loops or branch-heavy normalization logic.

## External Test Layout

Create `tests/internal/mod.rs` and move
`tests/lenient_json_normalizer_tests.rs` to
`tests/internal/lenient_json_normalizer_tests.rs`. Update `tests/mod.rs` to
load the `internal` test module. The tests will continue exercising internal
normalization behavior only through `LenientJsonDecoder`.

Dedicated test files will not be added for private `ControlCharacterEscaper`
or `MarkdownFence` types because their behavior is already observed through
the decoder and exposing them would weaken the crate boundary.

## Continuous Fuzzing

Add `.github/workflows/fuzz.yml` as an independent workflow:

- Trigger it on a daily schedule and `workflow_dispatch`.
- Check out submodules, install the repository's nightly Rust toolchain and
  `cargo-fuzz`, then run the decoder target for a bounded duration.
- Keep fuzzing out of pull-request CI so normal feedback latency is unchanged.
- Extend the target's decoder matrix to cover default, strict, JSON-only, and
  required-closing-fence configurations.
- Add the canonical copyright header to the fuzz target.

The workflow is a reliability check only; it does not alter published package
contents or runtime behavior.

## Documentation Alignment

Update `doc/json_design.zh_CN.md` and `doc/json_prd.zh_CN.md` so their source
and test inventories include:

- `json_decode_stage.rs`;
- `markdown_fence_closing.rs` and `markdown_fence_policy.rs`;
- all corresponding external test files;
- the nested `tests/internal/lenient_json_normalizer_tests.rs` path;
- benchmark, fuzz, and workflow validation assets where the document describes
  project structure and validation.

Behavioral descriptions remain unchanged.

## Verification

Run the repository-prescribed sequence from `rs-json` after implementation:

1. `./align-ci.sh`
2. `./ci-check.sh`
3. `./coverage.sh json` only if CI reports coverage below its threshold

Also build the fuzz target and inspect the final Git diff. Because alignment
may rewrite formatting, re-check all moved Rustdoc and attributes after
`align-ci.sh` before running CI.

## Non-goals

- No release-version bump.
- No downstream `rs-llmsdk-core` changes.
- No benchmark-result claim or performance redesign.
- No modification to normalization, parsing, deserialization, or error data
  flow.
