# rs-json Performance, Fuzz, Documentation, and Style Alignment Design

## Goal

Improve `qubit-json` without changing its public decoding contract: remove
avoidable work from successful typed object and array decoding, keep the
control-character repair state machine single-sourced, add repeatable
performance and fuzz coverage, and align documentation and Rust source layout
with the repository rules.

## Scope and Compatibility

- Keep every public type, method signature, preset, error kind, and default
  option unchanged.
- Preserve the error-ordering contract: malformed JSON remains `InvalidJson`
  even when its first token does not match `decode_object` or `decode_array`.
- Keep direct serde deserialization for typed object and array decoding, so
  duplicate-field rejection and exact integer handling remain unchanged.
- Do not add prose extraction, JSON repair beyond the existing rules, runtime
  dependencies, or provider-specific behavior.
- Add `criterion` only as a development dependency and a standard
  `cargo-fuzz` harness under `fuzz/`; neither is part of the published runtime
  dependency graph.

## Decoder Fast Path

`decode_with_top_level` currently validates every normalized object or array as
`RawValue` before deserializing it again. The revised sequence is:

1. Normalize the input exactly as today.
2. Classify its first JSON token.
3. If that token does not match the requested object or array kind, validate
   syntax with `RawValue`; return `InvalidJson` on failure and
   `UnexpectedTopLevel` only after successful validation.
4. If it matches, deserialize the requested type directly. Existing error
   classification maps syntax and EOF failures to `InvalidJson` and data
   failures to `Deserialize`.

This removes one complete parse from successful and type-mismatch object/array
decoding while retaining the PRD error precedence.

## Control-character Normalization

Replace the separate count and rewrite state machines with one scanner. It
tracks string and escape state while borrowing the input until it encounters
the first raw C0 character that must be rewritten. It then allocates one output
string, copies the untouched prefix, and continues using the same state
transitions. Existing odd/even-backslash semantics and valid escapes remain
unchanged.

The control-character escape mapping becomes a small state-independent helper
in an internal module. Markdown-fence recognition and control-character repair
also become separate internal modules so each private type is in its own
snake-case source file.

## Tests, Fuzzing, and Benchmarks

Integration tests continue to live under `tests/`. Add regression cases that
exercise matching and mismatching malformed top-level inputs, successful typed
object/array decoding, and the scanner's borrow-versus-own behavior through its
public decoder results.

Add a `fuzz/` cargo-fuzz package that feeds arbitrary UTF-8 text to all public
decode shapes and asserts they do not panic. Add Criterion benchmarks for
plain, fenced, and raw-control-character inputs across `decode`,
`decode_object`, `decode_array`, and `decode_value`; benchmarks report changes
but make no timing assertions in normal tests.

## Documentation and Style

Complete Rustdoc for public and private production functions, including
arguments, return values, errors, and the internal escape helper's panic
precondition. Synchronize README, PRD, design inventory, and package include
paths with the final source/test layout. Reorder inherent methods and apply the
repository's inline policy to thin accessors and forwarding methods.

## Validation

Run the repository-prescribed sequence after implementation:

1. `./align-ci.sh`
2. `./ci-check.sh`
3. `./coverage.sh json` only if CI reports coverage below its threshold

Also run the focused integration tests, documentation tests, Criterion compile
check, and the fuzz target's build/check command when the local toolchain makes
them available.
