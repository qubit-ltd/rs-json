# rs-json Benchmark, Fuzz, and Style Follow-up Design

## Goal

Improve `qubit-json` using measured performance evidence and a complete Rust
source-style audit, without changing decoding behavior, public signatures, or
the crate's deliberately narrow JSON-normalization scope.

## Scope and Compatibility

- Evaluate only `rs-json`; downstream crates are outside this work.
- Preserve every public type, method signature, option default, error category,
  normalization rule, and parsing semantic.
- Do not add JSONC parsing, prose extraction, streaming, NDJSON, or additional
  public entry points.
- Treat `jsonc` only as an accepted Markdown fence label. The fenced content
  must still be valid standard JSON accepted by `serde_json`.
- Do not move, delete, or add production source files unless a later structural
  finding is presented to and approved by the user.

## Evidence-first Control-character Benchmark

Extend the existing Criterion benchmark with JSON string payloads that isolate
the control-character normalization path at approximately 1 KiB and 64 KiB:

- plain strings containing no raw control characters;
- sparse strings containing a small number of raw C0 characters;
- dense strings containing repeated raw C0 characters that expand to
  `\\u00XX` escapes.

Build every payload before the timed loop and pass it through the public
`decode_value` entry point. This keeps the benchmark attached to shipped
behavior without widening private visibility solely for measurement.

Run the new benchmark against the current implementation first. Then trial an
exact-capacity strategy that preserves the current no-replacement single-pass
borrowed path: after the first required replacement is found, calculate the
remaining expansion, allocate the final capacity once, and perform the rewrite.

Keep that production optimization only when both dense cases show a
statistically significant improvement of at least 5%, while neither the plain
nor sparse 64 KiB case shows a statistically significant regression greater
than 3%. Criterion results are decision evidence only; no unstable timing
assertion is added to tests. If the trial does not meet these conditions,
restore the current implementation and change its type documentation to say
that it lazily creates one output `String`, without promising one heap
allocation.

## JSONC Documentation

Clarify the label-versus-grammar distinction in all user-facing and API-facing
locations that describe JSON-only fences:

- `README.md`;
- `README.zh_CN.md`;
- `MarkdownFencePolicy::JsonOnly`;
- `JsonDecodeOptions::json_code_fences_only`;
- the private fence-tag recognizer documentation.

Existing parsing remains strict `serde_json`. Add no compatibility parser and
change no accepted input behavior.

## Fuzz Reproducibility

Pin the scheduled workflow installation to `cargo-fuzz` version `0.13.2`, which
matches the locally available current tool. Add a small committed decoder
corpus covering:

- plain JSON;
- backtick- and tilde-fenced JSON;
- mixed CR/LF boundaries;
- raw control characters inside a JSON string;
- an unmatched trailing backslash;
- a JSONC-labelled fence whose content is standard JSON.

The fuzz invariant remains "no panic" across the existing decoder presets and
public decode shapes. The workflow duration and nightly toolchain stay
unchanged.

## Complete Rust Source-style Audit

Reinspect every Rust item under `src/` and apply the repository rules
semantically rather than mechanically:

- retain one struct or enum per snake-case source file and keep private helper
  types under `src/internal/`;
- ensure every type, field, function, and method has accurate Rustdoc and every
  applicable arguments, returns, errors, panics, safety, or example section;
- keep inherent constructors first, then methods by visibility and functional
  adjacency;
- use `#[inline(always)]` only for getters, setters, and pure forwarding;
- use `#[inline]` for other short helpers with little branching;
- omit inline attributes from loops, branch-heavy matches, and complex control
  flow;
- use `#[must_use]` for pure constructors, transformations, and queries whose
  unprotected result would otherwise be easy to discard accidentally;
- remove redundant `#[must_use]` attributes from methods returning `Option`,
  `Result`, or another return type that already carries the warning contract.

Known candidates from the initial inventory include the branch-heavy C0 escape
mapping, conditional normalization helpers marked `inline(always)`, missing
semantic `must_use` attributes on private pure functions, redundant attributes
on `Option` getters, and the undocumented private `normalizer` field. The final
edit set is determined by the complete item-by-item rescan. Style changes must
not alter runtime behavior or public paths.

## Testing and Validation

Benchmark additions are compiled before they are run. Documentation, workflow,
corpus, and attribute-only changes rely on the existing behavior suite; no
artificial runtime test is added for compiler hints. If a public warning
contract is newly introduced, add the compile-fail doctest required by the
style rules before applying the attribute.

After implementation, run the repository-prescribed sequence from the crate
root:

1. `./align-ci.sh`
2. `./ci-check.sh`
3. `./coverage.sh json` only if CI reports coverage below its threshold

Also compile documentation, compile the benchmark target, run the selected
before/after Criterion comparisons, build the fuzz target, and re-audit the
complete `src/` inventory and final diff.
