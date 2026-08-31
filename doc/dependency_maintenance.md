# JSON Dependency Maintenance

`qubit-json` uses the ordinary caret constraint `serde_json = "1.0.151"`.
`1.0.151` is the lowest version covered by compatibility tests, not the only
version allowed. The strict encoder must recognize serde_json's private
RawValue Serde protocol, so compatibility cannot be inferred from the version
number alone.

Finite floating-point JSON lexeme lengths are measured through the public
interface of `serde_json::ser::CompactFormatter`; this keeps the calculation
aligned with serde_json's actual formatter implementation. `zmij` is a
transitive serde_json dependency; `qubit-json` does not depend on or call it
directly.

## Upgrade procedure

1. Update the minimum `serde_json` version in `Cargo.toml` when necessary and
   update the lockfile.
2. Inspect the RawValue tokens and corresponding Serde shapes in
   `src/encode/serde_compat/`; confirm that the dependency graph does not
   enable `serde_json/arbitrary_precision`.
3. Run the RawValue, 64-bit number-boundary, and float-lexeme-length tests:

   ```bash
   cargo test --test tests serde_json_compat
   cargo test --test tests json_lexeme_length
   cargo test --test tests json_text_encoder
   ```

4. With the lockfile's resolved versions, run `./align-ci.sh`,
   `./style-check.sh`, and `./ci-check.sh`. Also compile the benchmarks, fuzz
   targets, and direct downstream crates. If validation fails, fix the
   compatibility layer or raise the minimum version; do not use an exact
   lockfile pin as a long-term substitute for compatibility.
5. Compare encoding results with
   `cargo bench --bench budgeted_serde_json`. If the RawValue protocol or the
   serde_json formatter behavior changes, update the compatibility
   implementation and regression tests before publishing the dependency
   upgrade.

See the [Chinese version](dependency_maintenance.zh_CN.md).
